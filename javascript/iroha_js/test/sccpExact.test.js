import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import test from "node:test";

import { keccak_256 } from "@noble/hashes/sha3";

import { AccountAddress } from "../src/address.js";
import { blake2b256 } from "../src/blake2b.js";
import {
  SCCP_CODEC_CANONICAL_TEXT,
  SCCP_CODEC_EVM_ADDRESS20,
  SCCP_CODEC_KEYS,
  SCCP_CODEC_SOLANA_PUBKEY32,
  SCCP_CODEC_TRON_ADDRESS21,
  SCCP_DOMAIN_SOLANA,
  SCCP_NETWORK_PROFILES,
  SCCP_PAYLOAD_KINDS,
  SCCP_SOLANA_TESTNET_GENESIS_HASH,
  deriveSccpSolanaDestinationHashesV1,
  deriveSccpSolanaNativeVerifierConfigHashV1,
  deriveSccpSolanaSourceIdentityHashesV1,
  normalizeBridgeMessageSubmitPayload,
  normalizeBridgeProofSubmitPayload,
  normalizeSccpBridgeSubmitResponse,
  normalizeSccpCapabilities,
  normalizeSccpCodecValue,
  normalizeSccpMessageBundle,
  normalizeSccpProofRequest,
  normalizeSccpRecentMessages,
  normalizeSccpRegistry,
  normalizeSccpRouteGovernanceAction,
  normalizeSccpSoraOutboundMaterial,
  parseSccpBridgeSubmitResponseJson,
  parseSccpJsonObject,
  sccpSourceEventDigest,
} from "../src/sccp.js";
import { ToriiClient } from "../src/toriiClient.js";

const HASH = (byte) => byte.toString(16).padStart(2, "0").repeat(32);
const PREFIX_HASH = (byte) => `0x${HASH(byte)}`;
const UPPER = (byte, length) => byte.toString(16).padStart(2, "0").repeat(length).toUpperCase();
const PUBLIC_KEY = Uint8Array.from([
  0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
  0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
  0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
  0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
]);
const ACCOUNT = AccountAddress.fromAccount({ publicKey: PUBLIC_KEY });
const AUTHORITY = ACCOUNT.toI105(369);
const MESSAGE_ID = HASH(0x11);
const MESSAGE_BUNDLE_NORITO_TYPE = "iroha_sccp::TairaSccpMessageProofV1";
const PROOF_REQUEST_NORITO_TYPE = "iroha_sccp::SccpGroth16Bn254ProofRequestV1";
const DESTINATION_PROOF_NORITO_TYPE =
  "iroha_sccp::SccpGroth16Bn254ProofArtifactV1";
const NATIVE_MESSAGE_PROOF_NORITO_TYPE =
  "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1";
const PUBLIC_SIGNAL_SCHEMA_HASH =
  "7567439F41173D6745A3D51923CB70371ACC7D66F23CEFB4100D6D5D7A432CBB";
const SORA_TAIRA_CHAIN_ID_HASH =
  "CF1CFC0F57B0BFA4C21882A9870317A1F4812F86533897095E3944BE34C5BBA7";

function b64(bytes) {
  return Buffer.from(bytes).toString("base64");
}

function network(profile) {
  return { network: profile.replaceAll("-", "_"), profile: null };
}

function lane(source = "bsc-mainnet") {
  return { source: network(source), target: network("sora-taira") };
}

function g1(x = 1, y = 2) {
  return { x: UPPER(x, 32), y: UPPER(y, 32) };
}

function g2(seed = 3) {
  return {
    x_c0: UPPER(seed, 32),
    x_c1: UPPER(seed + 1, 32),
    y_c0: UPPER(seed + 2, 32),
    y_c1: UPPER(seed + 3, 32),
  };
}

function verifyingKey() {
  const ic = { constant: g1() };
  for (let index = 0; index < 11; index += 1) ic[`signal_${index}`] = g1();
  return { version: 1, alpha1: g1(), beta2: g2(), gamma2: g2(), delta2: g2(), ic };
}

// Solana's native verifier material uses actual BN254 generator points. The
// older repeated-byte key above remains a structural parser fixture for the
// EVM/TVM tests, but must never stand in for deployable verifier material.
function solanaVerifyingKey() {
  const scalarWord = (value) => value.toString(16).padStart(64, "0").toUpperCase();
  const generatorG1 = { x: scalarWord(1), y: scalarWord(2) };
  const generatorG2 = {
    x_c0: "1800DEEF121F1E76426A00665E5C4479674322D4F75EDADD46DEBD5CD992F6ED",
    x_c1: "198E9393920D483A7260BFB731FB5D25F1AA493335A9E71297E485B7AEF312C2",
    y_c0: "12C85EA5DB8C6DEB4AAB71808DCB408FE3D1E7690C43D37B4CE6CC0166FA7DAA",
    y_c1: "090689D0585FF075EC9E99AD690C3395BC4B313370B38EF355ACDADCD122975B",
  };
  const ic = { constant: generatorG1 };
  for (let index = 0; index < 11; index += 1) ic[`signal_${index}`] = generatorG1;
  return {
    version: 1,
    alpha1: generatorG1,
    beta2: generatorG2,
    gamma2: generatorG2,
    delta2: generatorG2,
    ic,
  };
}

function verifyingKeyBytes(key) {
  const words = [];
  const addG1 = (point) => words.push(point.x, point.y);
  const addG2 = (point) => words.push(point.x_c0, point.x_c1, point.y_c0, point.y_c1);
  addG1(key.alpha1);
  addG2(key.beta2);
  addG2(key.gamma2);
  addG2(key.delta2);
  addG1(key.ic.constant);
  for (let index = 0; index < 11; index += 1) addG1(key.ic[`signal_${index}`]);
  return Uint8Array.from(Buffer.from(words.join(""), "hex"));
}

function keyHash(key) {
  return Buffer.from(keccak_256(verifyingKeyBytes(key))).toString("hex");
}

function semanticProfile() {
  return {
    profile: "sora_taira_finality_inclusion_groth16_bn254",
    commitments: {
      version: 1,
      circuit_commitment: UPPER(0xc1, 32),
      witness_generator_commitment: UPPER(0xc2, 32),
      public_signal_schema_hash: PUBLIC_SIGNAL_SCHEMA_HASH,
    },
  };
}

function finalityAnchor() {
  return {
    version: 1,
    source_network: network("sora-taira"),
    protocol_version: 3,
    chain_id_hash: SORA_TAIRA_CHAIN_ID_HASH,
    checkpoint_height: 7,
    checkpoint_block_hash: UPPER(0xa1, 32),
    checkpoint_context_id: UPPER(0xa2, 32),
    checkpoint_finality_artifact_hash: UPPER(0xa3, 32),
  };
}

function outboundPolicy() {
  return {
    version: 1,
    semantic_profile: semanticProfile(),
    sora_finality_anchor: finalityAnchor(),
  };
}

function soraOutboundExecutionPolicy() {
  return {
    version: 1,
    semantics: "ivm_proved_record_sccp_message_v1",
    contract_artifact_sha256: UPPER(0xb1, 32),
    vk_ref: {
      backend: "stark/fri/v1",
      name: "ivm-execution-v1",
      version: 1,
      commitment: UPPER(0xb2, 32),
    },
    gas_limit: 50_000_000,
  };
}

function policyHashes(policy = outboundPolicy()) {
  const semanticPolicy = policy.semantic_profile;
  const anchorPolicy = policy.sora_finality_anchor;
  const semantic = Buffer.from(
    keccak_256(
      Buffer.concat([
        Buffer.from("sccp:semantic-proof-profile:v1"),
        Buffer.from([1, 0, 1]),
        Buffer.from(semanticPolicy.commitments.circuit_commitment, "hex"),
        Buffer.from(semanticPolicy.commitments.witness_generator_commitment, "hex"),
        Buffer.from(semanticPolicy.commitments.public_signal_schema_hash, "hex"),
      ]),
    ),
  );
  const height = Buffer.alloc(8);
  height.writeBigUInt64LE(BigInt(anchorPolicy.checkpoint_height));
  const protocolVersion = Buffer.alloc(2);
  protocolVersion.writeUInt16LE(anchorPolicy.protocol_version);
  const anchor = Buffer.from(
    keccak_256(
      Buffer.concat([
        Buffer.from("sccp:sora-finality-anchor:v1"),
        Buffer.from([1, 1]),
        protocolVersion,
        Buffer.from(anchorPolicy.chain_id_hash, "hex"),
        height,
        Buffer.from(anchorPolicy.checkpoint_block_hash, "hex"),
        Buffer.from(anchorPolicy.checkpoint_context_id, "hex"),
        Buffer.from(anchorPolicy.checkpoint_finality_artifact_hash, "hex"),
      ]),
    ),
  );
  return { semantic: semantic.toString("hex"), anchor: anchor.toString("hex") };
}

function concatenate(...values) {
  return Buffer.concat(values.map((value) => Buffer.from(value)));
}

function littleEndian(value, width) {
  let remaining = BigInt(value);
  const result = Buffer.alloc(width);
  for (let index = 0; index < width; index += 1) {
    result[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  assert.equal(remaining, 0n);
  return result;
}

const NORITO_CRC64_TABLE = (() => {
  const table = [];
  for (let value = 0; value < 256; value += 1) {
    let crc = BigInt(value);
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 1n) !== 0n
        ? (crc >> 1n) ^ 0xc96c5795d7870f42n
        : crc >> 1n;
    }
    table.push(crc);
  }
  return table;
})();

function noritoCrc64Xz(payload) {
  let crc = 0xffffffffffffffffn;
  for (const byte of payload) {
    crc = NORITO_CRC64_TABLE[Number((crc ^ BigInt(byte)) & 0xffn)] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ 0xffffffffffffffffn);
}

function sccpNoritoFrame(typeName, { payload = Buffer.from([1, 2, 3, 4]), padding = 0 } = {}) {
  const schemaHash = createHash("sha256")
    .update(Buffer.from("norito:v1:type-name\0", "utf8"))
    .update(Buffer.from(typeName, "utf8"))
    .digest()
    .subarray(0, 16);
  return Buffer.concat([
    Buffer.from("NRT0", "ascii"),
    Buffer.from([0, 0]),
    schemaHash,
    Buffer.from([0]),
    littleEndian(payload.length, 8),
    littleEndian(noritoCrc64Xz(payload), 8),
    Buffer.from([0x02]),
    Buffer.alloc(padding),
    payload,
  ]);
}

function destinationProofB64(options) {
  return b64(sccpNoritoFrame(DESTINATION_PROOF_NORITO_TYPE, options));
}

function nativeProofB64(options) {
  return b64(sccpNoritoFrame(NATIVE_MESSAGE_PROOF_NORITO_TYPE, options));
}

function abiWord(value) {
  let remaining = BigInt(value);
  const result = Buffer.alloc(32);
  for (let index = 31; index >= 0 && remaining !== 0n; index -= 1) {
    result[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  assert.equal(remaining, 0n);
  return result;
}

function addressWord(value, tron = false) {
  const result = Buffer.alloc(32);
  if (tron) result[11] = 0x41;
  result.set(Buffer.from(value, "hex"), 12);
  return result;
}

const TEST_NETWORK_IDENTITIES = Object.freeze({
  "sora-taira": Object.freeze({ tag: 1, domain: 0, bytes: Buffer.from("fc56984b2be7431d840e21514d1883f0", "hex") }),
  "ethereum-mainnet": Object.freeze({ tag: 2, domain: 1, bytes: littleEndian(1, 8), routeId: "taira_eth_xor", id: 1 }),
  "ethereum-sepolia": Object.freeze({ tag: 3, domain: 1, bytes: littleEndian(11_155_111, 8), routeId: "taira_eth_xor", id: 11_155_111 }),
  "bsc-mainnet": Object.freeze({ tag: 4, domain: 2, bytes: littleEndian(56, 8), routeId: "taira_bsc_xor", id: 56 }),
  "bsc-testnet": Object.freeze({ tag: 5, domain: 2, bytes: littleEndian(97, 8), routeId: "taira_bsc_xor", id: 97 }),
  "tron-mainnet": Object.freeze({ tag: 10, domain: 5, bytes: littleEndian(0x2b66_53dc, 4), routeId: "taira_tron_xor", id: 0x2b66_53dc }),
  "tron-nile": Object.freeze({ tag: 11, domain: 5, bytes: littleEndian(0xcd86_90dc, 4), routeId: "taira_tron_xor", id: 0xcd86_90dc }),
  "tron-shasta": Object.freeze({ tag: 12, domain: 5, bytes: littleEndian(0x94a9_059e, 4), routeId: "taira_tron_xor", id: 0x94a9_059e }),
  "solana-testnet": Object.freeze({
    tag: 13,
    domain: 3,
    bytes: Buffer.from("3a132ece10305ec1830725502fa2b7e7eb8157e9123d4c1f654a71787161dc21", "hex"),
    routeId: "taira_sol_xor",
    id: null,
  }),
});

function canonicalNetwork(profile) {
  const descriptor = TEST_NETWORK_IDENTITIES[profile];
  return concatenate(
    Buffer.from([1, descriptor.tag]),
    littleEndian(descriptor.domain, 4),
    descriptor.bytes,
  );
}

function testLaneHash(source, target) {
  const sourceBytes = canonicalNetwork(source);
  const targetBytes = canonicalNetwork(target);
  return Buffer.from(
    blake2b256(
      concatenate(
        Buffer.from("sccp:lane-id:v1"),
        Buffer.from([1]),
        littleEndian(sourceBytes.length, 4),
        sourceBytes,
        littleEndian(targetBytes.length, 4),
        targetBytes,
      ),
    ),
  );
}

function testDestinationHashes(route) {
  const profile = route.lane_id.source.network.replaceAll("_", "-");
  const descriptor = TEST_NETWORK_IDENTITIES[profile];
  const deployment = route.destination.deployment;
  const tron = route.destination.family === "tron";
  const policy = policyHashes(deployment.outbound_proof_policy);
  const semanticHash = Buffer.from(policy.semantic, "hex");
  const anchorHash = Buffer.from(policy.anchor, "hex");
  const destinationBindingHash = Buffer.from(
    keccak_256(
      concatenate(
        keccak_256(Buffer.from(tron ? "iroha:sccp:tron-destination-binding:v1" : "iroha:sccp:evm-destination-binding:v1")),
        keccak_256(Buffer.from(tron ? "tron-groth16-bn254-v1" : "evm-groth16-bn254-v1")),
        abiWord(descriptor.id),
        abiWord(0),
        abiWord(descriptor.domain),
        addressWord(deployment.verifier_address, tron),
        addressWord(deployment.route_address, tron),
        Buffer.from(deployment.verifier_code_hash, "hex"),
        Buffer.from(deployment.verifier_key_hash, "hex"),
        semanticHash,
        anchorHash,
      ),
    ),
  );
  const deploymentWords = [
    addressWord(deployment.token_address),
    Buffer.from(deployment.token_code_hash, "hex"),
    addressWord(deployment.verifier_address),
    Buffer.from(deployment.verifier_code_hash, "hex"),
    Buffer.from(deployment.verifier_key_hash, "hex"),
    semanticHash,
    anchorHash,
  ];
  if (tron) deploymentWords.push(destinationBindingHash);
  const deploymentConfigHash = Buffer.from(keccak_256(concatenate(...deploymentWords)));
  const assetRouteConfigHash = Buffer.from(
    keccak_256(
      concatenate(
        keccak_256(Buffer.from("xor")),
        keccak_256(Buffer.from(descriptor.routeId)),
        abiWord(route.revision),
        abiWord(deployment.taira_to_token_multiplier),
      ),
    ),
  );
  const sourceLaneHash = testLaneHash(profile, "sora-taira");
  const destinationLaneHash = testLaneHash("sora-taira", profile);
  const routeConfigurationHash = Buffer.from(
    keccak_256(
      concatenate(
        keccak_256(Buffer.from("sccp:concrete-route-config:v1")),
        abiWord(descriptor.domain),
        abiWord(descriptor.tag),
        abiWord(descriptor.id),
        sourceLaneHash,
        destinationLaneHash,
        deploymentConfigHash,
        assetRouteConfigHash,
      ),
    ),
  );
  return Object.freeze({
    destinationBindingHash: destinationBindingHash.toString("hex").toUpperCase(),
    deploymentConfigHash: deploymentConfigHash.toString("hex").toUpperCase(),
    routeConfigurationHash: routeConfigurationHash.toString("hex").toUpperCase(),
  });
}

function capabilities() {
  return {
    version: 1,
    registry_revision: PREFIX_HASH(0x10),
    registry_path: "/v1/sccp/registry",
    message_bundle_path: "/v1/sccp/proofs/message/{message_id}",
    proof_request_path: "/v1/sccp/proof-requests/{message_id}",
    recent_messages_path: "/v1/sccp/messages/recent",
    sora_outbound_material_path:
      "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material",
    registry_limits: {
      max_governed_lanes: 16,
      max_live_governed_routes: 64,
      max_live_routes_per_lane: 8,
      max_retained_routes_per_lane: 64,
      max_retained_native_trust_anchors_per_lane: 4096,
    },
    resource_limits: {
      max_outbound_messages_per_block: 512,
      max_outbound_message_payload_bytes: 4096,
      max_pending_outbound_messages: 65_536,
      max_pending_outbound_payload_bytes: 256 * 1024 * 1024,
      max_proofs_per_transaction: 1,
      max_proofs_per_block: 4,
      max_proof_bytes_per_proof: 8 * 1024 * 1024,
      max_proof_bytes_per_transaction: 8 * 1024 * 1024,
      max_proof_bytes_per_block: 32 * 1024 * 1024,
      max_native_headers_per_transaction: 1004,
      max_native_headers_per_block: 4016,
      max_ethereum_light_client_updates_per_transaction: 128,
      max_ethereum_light_client_updates_per_block: 512,
      max_native_header_bytes_per_transaction: 8 * 1024 * 1024,
      max_native_header_bytes_per_block: 32 * 1024 * 1024,
      max_secp256k1_recoveries_per_transaction: 1005,
      max_secp256k1_recoveries_per_block: 4020,
      max_bls_aggregate_checks_per_transaction: 1004,
      max_bls_aggregate_checks_per_block: 4016,
      max_bls_signer_contributions_per_transaction: 131713,
      max_bls_signer_contributions_per_block: 526852,
      max_bn254_pairing_checks_per_transaction: 1,
      max_bn254_pairing_checks_per_block: 4,
    },
    proof_submit_path: "/v1/bridge/proofs/submit",
    native_message_submit_path: "/v1/bridge/messages",
  };
}

function soraOutboundMaterial() {
  const artifact = Buffer.from("governed-ivm-artifact-v1", "utf8");
  return {
    version: 1,
    registry_revision: PREFIX_HASH(0x10),
    route_key: {
      lane_id: lane("solana-testnet"),
      route_id: "taira_sol_xor",
      asset_key: "xor",
      revision: 1,
    },
    route_configuration_hash: PREFIX_HASH(0x20),
    destination_binding_hash: PREFIX_HASH(0x21),
    settlement_asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    policy: {
      version: 1,
      semantics: "ivm_proved_record_sccp_message_v1",
      contract_artifact_sha256: createHash("sha256").update(artifact).digest("hex").toUpperCase(),
      vk_ref: {
        backend: "stark/fri/v1",
        name: "ivm-execution-v1",
        version: 7,
        commitment: UPPER(0x23, 32),
      },
      gas_limit: 50_000_000,
    },
    contract_artifact_b64: artifact.toString("base64"),
    contract_code_hash: PREFIX_HASH(0x22),
    verifying_key_version: 7,
  };
}

function governedRoute({
  revision = 1,
  activation = "staged",
  source = "bsc-mainnet",
  inboundFinalityCutoff = null,
} = {}) {
  const key = verifyingKey();
  const routeAddress = UPPER(0x31, 20);
  const routeCodeHash = UPPER(0x41, 32);
  const family = source.startsWith("tron-") ? "tron" : "evm";
  const route = {
    lane_id: lane(source),
    route_id: TEST_NETWORK_IDENTITIES[source].routeId,
    asset_key: "xor",
    revision,
    activation: { activation, direction: null },
    inbound_finality_cutoff: inboundFinalityCutoff,
    source_identity: {
      lane: lane(source),
      emitter: {
        emitter: family,
        identity: {
          address: routeAddress,
          runtime_code_hash: routeCodeHash,
          route_config_hash: UPPER(0x42, 32),
        },
      },
    },
    destination: {
      family,
      deployment: {
        token_address: UPPER(0x11, 20),
        token_code_hash: UPPER(0x21, 32),
        verifier_address: UPPER(0x12, 20),
        verifier_code_hash: UPPER(0x22, 32),
        verifying_key: key,
        verifier_key_hash: keyHash(key).toUpperCase(),
        outbound_proof_policy: outboundPolicy(),
        route_address: routeAddress,
        route_code_hash: routeCodeHash,
        taira_to_token_multiplier: 1_000_000_000,
      },
    },
    sora_outbound_execution_policy: soraOutboundExecutionPolicy(),
    settlement: {
      asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      custody_account_id: AUTHORITY,
      payload_amount_scale: 9,
    },
  };
  route.source_identity.emitter.identity.route_config_hash =
    testDestinationHashes(route).routeConfigurationHash;
  return route;
}

function solanaDeployment() {
  const key = solanaVerifyingKey();
  const deployment = {
    token_mint_address: UPPER(0x11, 32),
    route_program_id: UPPER(0x12, 32),
    route_program_data_address: UPPER(0x13, 32),
    route_program_data_slot: 17,
    route_state_account: UPPER(0x14, 32),
    route_program_code_hash: UPPER(0x15, 32),
    native_verifier_program_id: UPPER(0x16, 32),
    native_verifier_program_data_address: UPPER(0x17, 32),
    native_verifier_program_data_slot: 18,
    native_verifier_material_account: UPPER(0x18, 32),
    native_verifier_program_code_hash: UPPER(0x19, 32),
    native_verifier_config_hash: UPPER(0x1a, 32),
    verifying_key: key,
    verifier_key_hash: keyHash(key).toUpperCase(),
    outbound_proof_policy: outboundPolicy(),
    taira_to_token_multiplier: 1,
  };
  deployment.native_verifier_config_hash =
    deriveSccpSolanaNativeVerifierConfigHashV1(deployment, UPPER(0x31, 32), 1)
      .slice(2)
      .toUpperCase();
  return deployment;
}

function solanaGovernedRoute({ activation = "staged" } = {}) {
  const deployment = solanaDeployment();
  const routeHashes = deriveSccpSolanaDestinationHashesV1(deployment, UPPER(0x31, 32), 1);
  return {
    lane_id: lane("solana-testnet"),
    route_id: "taira_sol_xor",
    asset_key: "xor",
    revision: 1,
    activation: { activation, direction: null },
    inbound_finality_cutoff: null,
    source_identity: {
      lane: lane("solana-testnet"),
      emitter: {
        emitter: "solana",
        identity: {
          program_id: UPPER(0x31, 32),
          program_data_address: UPPER(0x32, 32),
          program_data_slot: 19,
          state_account: UPPER(0x33, 32),
          program_code_hash: UPPER(0x34, 32),
          route_config_hash: routeHashes.route_configuration_hash.slice(2).toUpperCase(),
        },
      },
    },
    destination: { family: "solana", deployment },
    sora_outbound_execution_policy: soraOutboundExecutionPolicy(),
    settlement: {
      asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      custody_account_id: AUTHORITY,
      payload_amount_scale: 9,
    },
  };
}

function nativeTrustAnchor(source = "bsc-mainnet") {
  const backend = source.startsWith("tron-")
    ? "tron_dpos_v1"
    : source.startsWith("solana-")
      ? "solana_agave_v1"
    : source.startsWith("bsc-")
      ? "bsc_parlia_v1"
      : "ethereum_beacon_v1";
  return {
    backend: { backend, protocol: null },
    anchor_hash: UPPER(0x91, 32),
    checkpoint_height: 1,
  };
}

function registry(routes = [governedRoute()], anchor = null) {
  const anchors = anchor === null ? [] : [anchor];
  return {
    version: 1,
    lanes: [
      {
        lane_id: structuredClone(routes[0].lane_id),
        native_trust_anchors: anchors,
        current_native_trust_anchor_hash: anchors.at(-1)?.anchor_hash ?? null,
        routes,
      },
    ],
  };
}

function messageBundle() {
  return {
    version: 1,
    commitment_root: PREFIX_HASH(0x51),
    commitment: {
      version: 1,
      kind: "Transfer",
      context: {
        lane: {
          source: network("sora-taira"),
          target: network("bsc-mainnet"),
        },
        destination_binding_hash: PREFIX_HASH(0x52),
        route_configuration_hash: PREFIX_HASH(0x53),
      },
      message_id: PREFIX_HASH(0x54),
      payload_hash: PREFIX_HASH(0x55),
    },
    merkle_proof: { steps: [] },
    payload: {
      Transfer: {
        version: 1,
        source_domain: 0,
        dest_domain: 2,
        nonce: "7",
        route_revision: 1,
        asset_home_domain: 0,
        asset_id_codec: 1,
        asset_id: "0x786f72",
        amount: "1",
        sender_codec: 1,
        sender: "0x616c696365",
        recipient_codec: 2,
        recipient: `0x${HASH(0x21).slice(0, 40)}`,
        route_id_codec: 1,
        route_id: "0x74616972615f6273635f786f72",
      },
    },
    finality_proof: "0x0102",
  };
}

function proofRequest() {
  const key = verifyingKey();
  const policy = outboundPolicy();
  const hashes = policyHashes();
  return {
    version: 1,
    backend: { backend: "evm_groth16_bn254_v1", family: null },
    source_network: network("sora-taira"),
    target_network: network("bsc-mainnet"),
    public_inputs: {
      version: 1,
      message_id: PREFIX_HASH(0x11),
      payload_hash: PREFIX_HASH(0x12),
      target_domain: 2,
      commitment_root: PREFIX_HASH(0x13),
      finality_height: "9",
      finality_block_hash: PREFIX_HASH(0x14),
    },
    verifying_key: key,
    verifier_key_hash: `0x${keyHash(key)}`,
    semantic_proof_profile: policy.semantic_profile,
    semantic_proof_profile_hash: `0x${hashes.semantic}`,
    sora_finality_anchor: policy.sora_finality_anchor,
    sora_finality_anchor_hash: `0x${hashes.anchor}`,
    bundle_bytes: "0x0102",
    statement_hash: PREFIX_HASH(0x61),
    destination_binding_hash: PREFIX_HASH(0x62),
    route_configuration_hash: PREFIX_HASH(0x63),
    request_hash: PREFIX_HASH(0x64),
  };
}

function crossPolicyAliasedProofRequest() {
  const request = proofRequest();
  request.semantic_proof_profile.commitments.circuit_commitment =
    request.sora_finality_anchor.checkpoint_block_hash;
  request.semantic_proof_profile_hash = `0x${policyHashes({
    version: 1,
    semantic_profile: request.semantic_proof_profile,
    sora_finality_anchor: request.sora_finality_anchor,
  }).semantic}`;
  return request;
}

function recentItem(height = 9, id = MESSAGE_ID, commitmentIndex = 0) {
  return {
    height,
    commitment_index: commitmentIndex,
    message_id_hex: id,
    kind: "transfer",
    source_profile: "sora-taira",
    target_profile: "bsc-mainnet",
    destination_binding_hash: PREFIX_HASH(0x71),
    route_configuration_hash: PREFIX_HASH(0x72),
    target_domain: 2,
    asset_id: "xor",
    route_id: "taira_bsc_xor",
    recipient: null,
    amount: "1000",
    payload_projection: {
      Transfer: {
        version: 1,
        source_domain: 0,
        dest_domain: 2,
        nonce: "7",
        route_revision: 1,
        asset_home_domain: 0,
        asset_id: { CanonicalText: { value: "xor" } },
        amount: "1000",
        sender: { CanonicalText: { value: "alice@taira" } },
        recipient: { EvmAddress20: { bytes: `0x${"11".repeat(20)}` } },
        route_id: { CanonicalText: { value: "taira_bsc_xor" } },
      },
    },
    links: {
      bundle_path: `/v1/sccp/proofs/message/${id}`,
      proof_request_path: `/v1/sccp/proof-requests/${id}`,
    },
  };
}

function preparedResponse(overrides = {}) {
  const payload = Uint8Array.of(1, 2, 3, 4);
  const digest = Uint8Array.from(blake2b256(payload));
  digest[31] |= 1;
  return {
    submitted: false,
    payload_kind: "transfer",
    message_id_hex: MESSAGE_ID,
    backend: "bridge/sccp/native/bsc-parlia-v1",
    counterparty_domain: 2,
    counterparty_chain: "bsc-mainnet",
    route_configuration_hash_hex: HASH(0x31),
    range_start_height: 7,
    range_end_height: 9,
    creation_time_ms: 10,
    tx_hash_hex: null,
    transaction_payload_b64: b64(payload),
    signing_message_b64: b64(digest),
    ...overrides,
  };
}

test("closed SCCP inventory exposes exact ETH, BSC, Solana testnet, and TRON profiles", async () => {
  assert.deepEqual(Object.keys(SCCP_NETWORK_PROFILES), [
    "sora-taira",
    "ethereum-mainnet",
    "ethereum-sepolia",
    "bsc-mainnet",
    "bsc-testnet",
    "tron-mainnet",
    "tron-nile",
    "tron-shasta",
    "solana-testnet",
  ]);
  assert.equal(Object.values(SCCP_NETWORK_PROFILES).some(({ tag }) => tag === 0), false);
  assert.deepEqual(Object.keys(SCCP_CODEC_KEYS), ["1", "2", "5", "6"]);
  assert.deepEqual(SCCP_NETWORK_PROFILES["solana-testnet"], {
    profile: "solana-testnet",
    tag: 13,
    domain: SCCP_DOMAIN_SOLANA,
    sora: false,
    genesisHash: SCCP_SOLANA_TESTNET_GENESIS_HASH,
  });
  assert.deepEqual(SCCP_PAYLOAD_KINDS, ["transfer"]);
  const exports = await import("../src/sccp.js");
  for (const retired of [
    "SCCP_DOMAIN_SOL",
    "SCCP_DOMAIN_TON",
    "SCCP_CODEC_SOLANA_BASE58",
    "SCCP_CODEC_TON_ACCOUNT36",
    "SCCP_CODEC_SORA_ASSET_ID",
    "normalizeSccpProofManifests",
    "normalizeSccpSourceAdapterEngineDeployment",
  ]) {
    assert.equal(retired in exports, false, retired);
  }
});

test("closed codecs accept exact layouts and reject retired tags and textual aliases", () => {
  assert.deepEqual(normalizeSccpCodecValue(1, "merchant@taira"), new TextEncoder().encode("merchant@taira"));
  assert.match(AUTHORITY, /[^\x00-\x7f]/u, "fixture must exercise non-ASCII I105 digits");
  assert.deepEqual(normalizeSccpCodecValue(1, AUTHORITY), new TextEncoder().encode(AUTHORITY));
  assert.equal(normalizeSccpCodecValue(2, new Uint8Array(20).fill(1)).length, 20);
  assert.equal(
    normalizeSccpCodecValue(5, Uint8Array.from([0x41, ...new Uint8Array(20).fill(2)])).length,
    21,
  );
  assert.equal(normalizeSccpCodecValue(6, new Uint8Array(32).fill(3)).length, 32);
  assert.equal(SCCP_CODEC_SOLANA_PUBKEY32, 6);
  for (const [tag, value] of [
    [3, new Uint8Array(32).fill(1)],
    [4, new Uint8Array(36).fill(1)],
    [6, Uint8Array.of(1)],
    [6, new Uint8Array(32)],
    [6, "11111111111111111111111111111111"],
    [2, `0x${"11".repeat(20)}`],
    [2, new Uint8Array(20)],
    [5, Uint8Array.from([0x42, ...new Uint8Array(20).fill(1)])],
    [1, " padded"],
    [1, "contains space"],
    [1, "line\nbreak"],
    [1, "merchant\ud83d\ude42"],
    [1, `${AUTHORITY.slice(0, -1)}${AUTHORITY.endsWith("1") ? "2" : "1"}`],
    [1, `n369${AUTHORITY.slice("test".length)}`],
    [1, `${AUTHORITY}${"\uff72".repeat(100)}`],
  ]) assert.throws(() => normalizeSccpCodecValue(tag, value));
});

test("source-event digest matches all shared ETH/BSC/TRON vectors", () => {
  const fixture = JSON.parse(
    fs.readFileSync(new URL("../../../fixtures/sccp/native_transfer_event_v1.json", import.meta.url), "utf8"),
  );
  for (const vector of fixture.vectors) {
    assert.equal(
      sccpSourceEventDigest(vector.lane_hash_hex, vector.message_id_hex, vector.payload_hash_hex),
      vector.source_event_digest_hex,
    );
  }
  for (const roles of [
    ["00".repeat(32), HASH(2), HASH(3)],
    [HASH(1), HASH(1), HASH(3)],
    [`0x${HASH(1)}`, HASH(2), HASH(3)],
    ["ab".repeat(32).toUpperCase(), HASH(2), HASH(3)],
  ]) assert.throws(() => sccpSourceEventDigest(...roles));
});

test("capabilities require exact immutable paths and reject all retired discovery fields", () => {
  const parsed = normalizeSccpCapabilities(capabilities());
  assert.equal(parsed.proof_request_path, capabilities().proof_request_path);
  assert.equal(
    parsed.sora_outbound_material_path,
    capabilities().sora_outbound_material_path,
  );
  assert.equal(parsed.registry_limits.max_retained_routes_per_lane, 64);
  assert.equal(parsed.registry_limits.max_retained_native_trust_anchors_per_lane, 4096);
  assert.equal(parsed.resource_limits.max_outbound_messages_per_block, 512);
  assert.equal(parsed.resource_limits.max_outbound_message_payload_bytes, 4096);
  assert.equal(parsed.resource_limits.max_pending_outbound_messages, 65_536);
  assert.equal(parsed.resource_limits.max_bls_signer_contributions_per_transaction, 131713);
  const readOnly = capabilities();
  delete readOnly.proof_submit_path;
  delete readOnly.native_message_submit_path;
  assert.equal(normalizeSccpCapabilities(readOnly).proof_submit_path, null);
  const mutations = [
    (value) => { value.registry_path = "/v1/sccp/manifests"; },
    (value) => { value.proof_request_path += "?network=bsc"; },
    (value) => { value.message_bundle_path = "/v1/sccp/proofs/message/{id}"; },
    (value) => { value.sora_outbound_material_path += "?bytecode=caller-selected"; },
    (value) => { value.proof_artifact_path = "/v1/sccp/artifacts/message/{message_id}"; },
    (value) => { value.proof_job_path = "/v1/sccp/jobs/message/{message_id}"; },
    (value) => { value.outbound = {}; },
    (value) => { value.allow_unready = true; },
    (value) => { value.registry_revision = PREFIX_HASH(0); },
    (value) => { delete value.proof_submit_path; },
    (value) => { delete value.native_message_submit_path; },
  ];
  for (const mutate of mutations) {
    const value = structuredClone(capabilities());
    mutate(value);
    assert.throws(() => normalizeSccpCapabilities(value));
  }
  for (const field of Object.keys(capabilities().resource_limits)) {
    const value = structuredClone(capabilities());
    value.resource_limits[field] = 0;
    assert.throws(() => normalizeSccpCapabilities(value), new RegExp(field, "u"));
  }
  const driftedRegistryLimits = structuredClone(capabilities());
  driftedRegistryLimits.registry_limits.max_retained_routes_per_lane = 65;
  assert.throws(() => normalizeSccpCapabilities(driftedRegistryLimits), /fixed V1 capacities/u);
  for (const [field, value] of [
    ["max_outbound_messages_per_block", 511],
    ["max_outbound_messages_per_block", 513],
    ["max_outbound_message_payload_bytes", 4095],
    ["max_outbound_message_payload_bytes", 4097],
  ]) {
    const drifted = structuredClone(capabilities());
    drifted.resource_limits[field] = value;
    assert.throws(() => normalizeSccpCapabilities(drifted), /fixed V1 capacities/u);
  }
});

test("route-scoped SORA outbound material binds exact route, policy, and artifact digest", () => {
  const material = soraOutboundMaterial();
  const parsed = normalizeSccpSoraOutboundMaterial(material, {
    sourceProfile: "solana-testnet",
    routeId: "taira_sol_xor",
    assetKey: "xor",
    revision: 1,
    registryRevision: PREFIX_HASH(0x10),
  });
  assert.equal(parsed.policy.gas_limit, 50_000_000);
  assert.equal(parsed.contract_artifact_b64, material.contract_artifact_b64);

  const mutations = [
    (value) => { value.contract_artifact_b64 = Buffer.from("caller-selected").toString("base64"); },
    (value) => { value.policy.gas_limit = 0; },
    (value) => { value.policy.vk_ref.backend = "stark//fri"; },
    (value) => { value.policy.vk_ref.version = 0; },
    (value) => { value.policy.vk_ref.version += 1; },
    (value) => { value.policy.vk_ref.commitment = UPPER(0, 32); },
    (value) => { value.verifying_key_version += 1; },
    (value) => { value.policy.bytecode = value.contract_artifact_b64; },
    (value) => { value.route_key.route_id = "legacy_sol_xor"; },
    (value) => { value.route_key.legacy_route = "taira_sol_xor"; },
    (value) => { value.settlement_asset_definition_id = "xor#universal"; },
    (value) => { value.destination_binding_hash = value.route_configuration_hash; },
    (value) => { value.contract_artifact = value.contract_artifact_b64; },
  ];
  for (const mutate of mutations) {
    const hostile = structuredClone(material);
    mutate(hostile);
    assert.throws(
      () =>
        normalizeSccpSoraOutboundMaterial(hostile, {
          sourceProfile: "solana-testnet",
          routeId: "taira_sol_xor",
          assetKey: "xor",
          revision: 1,
        }),
    );
  }
  assert.throws(
    () => normalizeSccpSoraOutboundMaterial(material, { routeId: "alias_sol_xor" }),
    /requested route context/u,
  );
});

test("capabilities reject every reversed per-proof, transaction, and block limit relation", () => {
  const orderedRelations = [
    ["max_proof_bytes_per_proof", "max_proof_bytes_per_transaction", /per-proof byte limit/u],
    ["max_proofs_per_transaction", "max_proofs_per_block", /transaction resource limits/u],
    ["max_proof_bytes_per_transaction", "max_proof_bytes_per_block", /transaction resource limits/u],
    ["max_native_headers_per_transaction", "max_native_headers_per_block", /transaction resource limits/u],
    ["max_ethereum_light_client_updates_per_transaction", "max_ethereum_light_client_updates_per_block", /transaction resource limits/u],
    ["max_native_header_bytes_per_transaction", "max_native_header_bytes_per_block", /transaction resource limits/u],
    ["max_secp256k1_recoveries_per_transaction", "max_secp256k1_recoveries_per_block", /transaction resource limits/u],
    ["max_bls_aggregate_checks_per_transaction", "max_bls_aggregate_checks_per_block", /transaction resource limits/u],
    ["max_bls_signer_contributions_per_transaction", "max_bls_signer_contributions_per_block", /transaction resource limits/u],
    ["max_bn254_pairing_checks_per_transaction", "max_bn254_pairing_checks_per_block", /transaction resource limits/u],
  ];
  for (const [lowerField, upperField, expected] of orderedRelations) {
    const reversed = structuredClone(capabilities());
    reversed.resource_limits[lowerField] = reversed.resource_limits[upperField] + 1;
    assert.throws(
      () => normalizeSccpCapabilities(reversed),
      expected,
      `${lowerField} must not exceed ${upperField}`,
    );
  }
});

test("capability integers preserve canonical JSON tokens and the shared exact range", () => {
  const canonical = JSON.stringify(capabilities());
  const needle = '"max_proofs_per_transaction":1';
  assert.ok(canonical.includes(needle));
  for (const token of ["1.0", "1e0", "-0", "9007199254740992.5", "1e999"]) {
    const hostile = canonical.replace(needle, `"max_proofs_per_transaction":${token}`);
    assert.throws(() => parseSccpJsonObject(hostile, "SCCP capabilities"), token);
  }

  const boundary = structuredClone(capabilities());
  for (const field of [
    "max_pending_outbound_messages",
    "max_pending_outbound_payload_bytes",
    "max_proof_bytes_per_proof",
    "max_proof_bytes_per_transaction",
    "max_proof_bytes_per_block",
    "max_native_header_bytes_per_transaction",
    "max_native_header_bytes_per_block",
  ]) boundary.resource_limits[field] = Number.MAX_SAFE_INTEGER;
  assert.equal(
    normalizeSccpCapabilities(boundary).resource_limits.max_proof_bytes_per_block,
    Number.MAX_SAFE_INTEGER,
  );
  boundary.resource_limits.max_proof_bytes_per_block = Number.MAX_SAFE_INTEGER + 1;
  assert.throws(() => normalizeSccpCapabilities(boundary), /safe integer/u);
  for (const field of [
    "max_pending_outbound_messages",
    "max_pending_outbound_payload_bytes",
  ]) {
    const oversized = structuredClone(capabilities());
    oversized.resource_limits[field] = Number.MAX_SAFE_INTEGER + 1;
    assert.throws(() => normalizeSccpCapabilities(oversized), /safe integer/u);
  }
});

test("registry checks retained-history caps before traversing attacker-controlled entries", () => {
  const exactAnchors = registry();
  exactAnchors.lanes[0].native_trust_anchors = Array(4096).fill(null);
  assert.throws(
    () => normalizeSccpRegistry(exactAnchors),
    (error) => !/more than 4,096/u.test(error.message),
  );
  const overAnchors = registry();
  overAnchors.lanes[0].native_trust_anchors = Array(4097).fill(null);
  assert.throws(() => normalizeSccpRegistry(overAnchors), /more than 4,096/u);

  const exactRoutes = registry();
  exactRoutes.lanes[0].routes = Array(64).fill({});
  assert.throws(
    () => normalizeSccpRegistry(exactRoutes),
    (error) => !/more than 64 retained/u.test(error.message),
  );
  const overRoutes = registry();
  overRoutes.lanes[0].routes = Array(65).fill({});
  assert.throws(() => normalizeSccpRegistry(overRoutes), /more than 64 retained/u);
});

test("registry validates complete typed route identity and immutable key hash", () => {
  const parsed = normalizeSccpRegistry(registry());
  assert.equal(parsed.lanes.length, 1);
  assert.equal(Object.isFrozen(parsed.lanes[0]), true);
  assert.equal(normalizeSccpRegistry(registry([governedRoute({ source: "tron-mainnet" })])).lanes.length, 1);
  const badHash = registry();
  badHash.lanes[0].routes[0].destination.deployment.verifier_key_hash = UPPER(0x99, 32);
  assert.throws(() => normalizeSccpRegistry(badHash), /verifier_key_hash/u);
  const alias = registry();
  alias.lanes[0].routes[0].destination.deployment.verifier_address =
    alias.lanes[0].routes[0].destination.deployment.token_address;
  assert.throws(() => normalizeSccpRegistry(alias), /reuses/u);
  const tenSignal = registry();
  delete tenSignal.lanes[0].routes[0].destination.deployment.verifying_key.ic.signal_10;
  assert.throws(() => normalizeSccpRegistry(tenSignal), /signal_10/u);
  const policyless = registry();
  delete policyless.lanes[0].routes[0].destination.deployment.outbound_proof_policy;
  assert.throws(() => normalizeSccpRegistry(policyless), /outbound_proof_policy/u);
  const missingExecutionPolicy = registry();
  delete missingExecutionPolicy.lanes[0].routes[0].sora_outbound_execution_policy;
  assert.throws(
    () => normalizeSccpRegistry(missingExecutionPolicy),
    /sora_outbound_execution_policy/u,
  );
  for (const field of ["version", "commitment"]) {
    const missingPin = registry();
    delete missingPin.lanes[0].routes[0].sora_outbound_execution_policy.vk_ref[field];
    assert.throws(() => normalizeSccpRegistry(missingPin), new RegExp(field, "u"));
  }
  const aliasedVkCommitment = registry();
  aliasedVkCommitment.lanes[0].routes[0].sora_outbound_execution_policy.vk_ref.commitment =
    aliasedVkCommitment.lanes[0].routes[0].sora_outbound_execution_policy.contract_artifact_sha256;
  assert.throws(() => normalizeSccpRegistry(aliasedVkCommitment), /reuses/u);
  const wrongSettlementAsset = registry();
  wrongSettlementAsset.lanes[0].routes[0].settlement.asset_definition_id =
    "another-canonical-looking-asset";
  assert.throws(
    () => normalizeSccpRegistry(wrongSettlementAsset),
    /first-release Taira XOR asset/u,
  );
});

test("registry destination hashes match the canonical Rust EVM and TRON layouts", () => {
  const vectors = [
    {
      source: "bsc-mainnet",
      destinationBindingHash: "CF29FF20DED900EE5571D1D2DED8CD14C85018FD63AF0DA89A040B4BFDE30280",
      deploymentConfigHash: "50542CF770B037DC5762D23945B3F7985E41BA0D431DAA92EBEF76A2313F021E",
      routeConfigurationHash: "57F92589F513D0DDA3EDB5BAAF7490B32937320971D7DC4EFC579ABD1E84787D",
    },
    {
      source: "tron-mainnet",
      destinationBindingHash: "229E90F2529AC2726DCB4294A938F695678808B3C6534251A07869166EAD0DAA",
      deploymentConfigHash: "C799A8D172845B3D7E15BE95A119EEA0B798666186F0250BE1CCD472EB68F664",
      routeConfigurationHash: "5475A14BFDBF9E8726B61BE7CE544F5775E0EBE87A607283DEC82E6B347E4760",
    },
  ];
  for (const vector of vectors) {
    const route = governedRoute({ source: vector.source });
    assert.deepEqual(testDestinationHashes(route), {
      destinationBindingHash: vector.destinationBindingHash,
      deploymentConfigHash: vector.deploymentConfigHash,
      routeConfigurationHash: vector.routeConfigurationHash,
    });
    assert.equal(
      route.source_identity.emitter.identity.route_config_hash,
      vector.routeConfigurationHash,
    );
    assert.equal(normalizeSccpRegistry(registry([route])).lanes.length, 1);
  }
});

test("Solana registry hashes match Rust and bind every Loader-v3 role", () => {
  const deployment = solanaDeployment();
  assert.equal(
    deriveSccpSolanaNativeVerifierConfigHashV1(deployment, UPPER(0x31, 32), 1),
    "0xbcb83baf2f2ab57a56b72529cf749da6175f8e65a048287eae217b61a2c84669",
  );
  const hashes = deriveSccpSolanaDestinationHashesV1(deployment, UPPER(0x31, 32), 1);
  assert.deepEqual(hashes, {
    destination_binding_hash:
      "0xcd1ff581301bd31b583b835ec71f185139ce1af2376dfe656216481f7a77ba2c",
    deployment_config_hash:
      "0x39256215e4432d59fc8a9ff0f89db0027f7256cae0fdef10179fba89612c6473",
    route_configuration_hash:
      "0x3f2c81fe59637d4a9af916dfce1b623ef59f44087db3ee0c25e42ad8ec1bf958",
  });
  const route = solanaGovernedRoute();
  assert.deepEqual(deriveSccpSolanaSourceIdentityHashesV1(route.source_identity), {
    source_emitter_identity_hash:
      "0xf0c6b976d69c3d0e001b5ee87d7d2fabd068db424c1e261cf8e9e1d8b1f4cbfa",
    source_identity_hash:
      "0x6c62bd033e5beb7848c66c10ae1be0a6fc1960b239f7b04b31bb3c5a7b1efa69",
  });
  assert.equal(normalizeSccpRegistry(registry([route])).lanes.length, 1);

  const mutations = [
    ["token_mint_address", "route_program_id"],
    ["route_program_data_address", "route_state_account"],
    ["route_program_code_hash", "native_verifier_config_hash"],
    ["native_verifier_program_id", "native_verifier_program_data_address"],
    ["native_verifier_material_account", "verifier_key_hash"],
  ];
  for (const [left, right] of mutations) {
    const hostile = structuredClone(deployment);
    hostile[left] = hostile[right];
    assert.throws(
      () => deriveSccpSolanaDestinationHashesV1(hostile, UPPER(0x31, 32), 1),
      /reuses/u,
      `${left} must not alias ${right}`,
    );
  }
  for (const slot of ["route_program_data_slot", "native_verifier_program_data_slot"]) {
    const hostile = structuredClone(deployment);
    hostile[slot] = 0;
    assert.throws(
      () => deriveSccpSolanaDestinationHashesV1(hostile, UPPER(0x31, 32), 1),
      /integer/u,
    );
  }
  const configExcludedMutations = [
    ["route_program_data_address", UPPER(0x41, 32)],
    ["route_program_data_slot", 41],
    ["route_program_code_hash", UPPER(0x42, 32)],
    ["native_verifier_program_data_address", UPPER(0x43, 32)],
    ["native_verifier_program_data_slot", 43],
    ["native_verifier_material_account", UPPER(0x44, 32)],
    ["native_verifier_program_code_hash", UPPER(0x45, 32)],
    ["native_verifier_config_hash", UPPER(0x46, 32)],
  ];
  const exactConfig = deriveSccpSolanaNativeVerifierConfigHashV1(
    deployment,
    UPPER(0x31, 32),
    1,
  );
  for (const [field, value] of configExcludedMutations) {
    const changed = structuredClone(deployment);
    changed[field] = value;
    assert.equal(
      deriveSccpSolanaNativeVerifierConfigHashV1(changed, UPPER(0x31, 32), 1),
      exactConfig,
      `${field} must remain outside the one-way material config preimage`,
    );
  }
  for (const [field, value] of [
    ["token_mint_address", UPPER(0x47, 32)],
    ["route_program_id", UPPER(0x48, 32)],
    ["route_state_account", UPPER(0x49, 32)],
    ["native_verifier_program_id", UPPER(0x4a, 32)],
  ]) {
    const changed = structuredClone(deployment);
    changed[field] = value;
    assert.notEqual(
      deriveSccpSolanaNativeVerifierConfigHashV1(changed, UPPER(0x31, 32), 1),
      exactConfig,
      `${field} must be committed by the material config`,
    );
  }
  assert.throws(
    () => deriveSccpSolanaNativeVerifierConfigHashV1(deployment, UPPER(0x31, 32), 2),
    /integer/u,
    "the exact first-release Solana deployment must reject revision two",
  );
  assert.throws(
    () =>
      deriveSccpSolanaDestinationHashesV1(deployment, UPPER(0x31, 32), 0xffff_ffff),
    /integer/u,
  );
  const staleConfig = structuredClone(deployment);
  staleConfig.route_state_account = UPPER(0x4b, 32);
  assert.throws(
    () => deriveSccpSolanaDestinationHashesV1(staleConfig, UPPER(0x31, 32), 1),
    /native_verifier_config_hash/u,
  );
  const stale = solanaGovernedRoute();
  stale.destination.deployment.route_state_account = UPPER(0x44, 32);
  stale.destination.deployment.native_verifier_config_hash =
    deriveSccpSolanaNativeVerifierConfigHashV1(
      stale.destination.deployment,
      stale.source_identity.emitter.identity.program_id,
      1,
    )
      .slice(2)
      .toUpperCase();
  assert.throws(() => normalizeSccpRegistry(registry([stale])), /route_config_hash/u);
  const independentlyGoverned = solanaGovernedRoute();
  assert.notEqual(
    independentlyGoverned.source_identity.emitter.identity.program_id,
    independentlyGoverned.destination.deployment.route_program_id,
  );
  assert.equal(normalizeSccpRegistry(registry([independentlyGoverned])).lanes.length, 1);
  for (const [sourceField, destinationField] of [
    ["program_id", "route_program_id"],
    ["program_data_address", "route_program_data_address"],
    ["state_account", "route_state_account"],
    ["program_code_hash", "route_program_code_hash"],
  ]) {
    const aliased = solanaGovernedRoute();
    aliased.source_identity.emitter.identity[sourceField] =
      aliased.destination.deployment[destinationField];
    assert.throws(
      () => normalizeSccpRegistry(registry([aliased])),
      /(?:reuses a destination program role|aliases a Solana source role)/u,
      `${sourceField} must remain distinct from destination ${destinationField}`,
    );
  }
});

test("registry rejects stale emitter hashes after either typed proof policy changes", () => {
  const mutations = [
    {
      label: "semantic profile",
      mutate(policy) {
        policy.semantic_profile.commitments.circuit_commitment = UPPER(0xc3, 32);
      },
      changed(before, after) {
        return before.semantic !== after.semantic && before.anchor === after.anchor;
      },
    },
    {
      label: "Taira finality anchor",
      mutate(policy) {
        policy.sora_finality_anchor.checkpoint_height += 1;
      },
      changed(before, after) {
        return before.semantic === after.semantic && before.anchor !== after.anchor;
      },
    },
  ];
  for (const source of ["bsc-mainnet", "tron-mainnet"]) {
    for (const mutation of mutations) {
      const route = governedRoute({ source });
      const policy = route.destination.deployment.outbound_proof_policy;
      const before = policyHashes(policy);
      mutation.mutate(policy);
      const after = policyHashes(policy);
      assert.equal(mutation.changed(before, after), true, mutation.label);
      assert.throws(
        () => normalizeSccpRegistry(registry([route])),
        /route_config_hash does not match/u,
        `${source} must reject a stale emitter after changing its ${mutation.label}`,
      );
    }
  }
});

test("registry rejects legacy and ambiguous Sumeragi v2 finality anchors", () => {
  const mutations = [
    ["wrong protocol", (anchor) => { anchor.protocol_version = 1; }, /protocol_version/u],
    ["protocol type confusion", (anchor) => { anchor.protocol_version = true; }, /integer/u],
    ["zero context", (anchor) => { anchor.checkpoint_context_id = UPPER(0, 32); }, /nonzero/u],
    ["aliased artifact", (anchor) => {
      anchor.checkpoint_finality_artifact_hash = anchor.checkpoint_context_id;
    }, /consensus hash role/u],
    ["legacy validator fields", (anchor) => { anchor.validator_set_epoch = 2; }, /field/u],
  ];
  for (const [label, mutate, pattern] of mutations) {
    const route = governedRoute({ source: "bsc-mainnet" });
    const anchor = route.destination.deployment.outbound_proof_policy.sora_finality_anchor;
    mutate(anchor);
    assert.throws(
      () => normalizeSccpRegistry(registry([route])),
      pattern,
      label,
    );
  }
});

test("registry rejects every stale route-configuration intermediary", () => {
  const mutations = [
    ["token address", (route) => { route.destination.deployment.token_address = UPPER(0x13, 20); }],
    ["token code", (route) => { route.destination.deployment.token_code_hash = UPPER(0x23, 32); }],
    ["verifier address", (route) => { route.destination.deployment.verifier_address = UPPER(0x14, 20); }],
    ["verifier code", (route) => { route.destination.deployment.verifier_code_hash = UPPER(0x24, 32); }],
    ["verifying key", (route) => {
      route.destination.deployment.verifying_key.alpha1 = g1(7, 8);
      route.destination.deployment.verifier_key_hash =
        keyHash(route.destination.deployment.verifying_key).toUpperCase();
    }],
    ["route revision", (route) => { route.revision += 1; }],
  ];
  for (const source of ["bsc-mainnet", "tron-mainnet"]) {
    for (const [label, mutate] of mutations) {
      const route = governedRoute({ source });
      const stale = route.source_identity.emitter.identity.route_config_hash;
      mutate(route);
      assert.notEqual(testDestinationHashes(route).routeConfigurationHash, stale, label);
      assert.throws(
        () => normalizeSccpRegistry(registry([route])),
        /route_config_hash does not match/u,
        `${source} must reject a stale ${label} intermediary`,
      );
    }
  }
  for (const source of ["bsc-mainnet", "tron-mainnet"]) {
    const route = governedRoute({ source });
    route.source_identity.emitter.identity.route_config_hash = UPPER(0xfe, 32);
    assert.throws(
      () => normalizeSccpRegistry(registry([route])),
      /route_config_hash does not match/u,
    );
  }
  const tronRoute = governedRoute({ source: "tron-mainnet" });
  const before = testDestinationHashes(tronRoute);
  tronRoute.destination.deployment.route_address = UPPER(0x32, 20);
  tronRoute.source_identity.emitter.identity.address = UPPER(0x32, 20);
  const after = testDestinationHashes(tronRoute);
  assert.notEqual(after.destinationBindingHash, before.destinationBindingHash);
  assert.notEqual(after.deploymentConfigHash, before.deploymentConfigHash);
  assert.notEqual(after.routeConfigurationHash, before.routeConfigurationHash);
  assert.throws(
    () => normalizeSccpRegistry(registry([tronRoute])),
    /route_config_hash does not match/u,
  );
});

test("registry requires a native trust anchor for every inbound-enabled route", () => {
  for (const activation of ["bidirectional", "inbound_only"]) {
    for (const source of ["bsc-mainnet", "tron-mainnet"]) {
      const route = governedRoute({ activation, source });
      assert.throws(
        () => normalizeSccpRegistry(registry([route])),
        /without a trust anchor/u,
      );
      assert.equal(
        normalizeSccpRegistry(registry([route], nativeTrustAnchor(source))).lanes.length,
        1,
      );
    }
  }
  for (const activation of ["bidirectional", "inbound_only"]) {
    const route = solanaGovernedRoute({ activation });
    assert.throws(
      () => normalizeSccpRegistry(registry([route])),
      /without a trust anchor/u,
    );
    assert.equal(
      normalizeSccpRegistry(
        registry([route], nativeTrustAnchor("solana-testnet")),
      ).lanes.length,
      1,
    );
  }
});

test("registry keeps old staging profiles fail-closed while allowing exact Solana testnet", () => {
  for (const source of ["ethereum-sepolia", "bsc-testnet", "tron-nile", "tron-shasta"]) {
    const route = governedRoute({ activation: "bidirectional", source });
    assert.throws(
      () => normalizeSccpRegistry(registry([route], nativeTrustAnchor(source))),
      /unapproved staging profile/u,
    );
  }
  const solana = solanaGovernedRoute({ activation: "bidirectional" });
  assert.equal(
    normalizeSccpRegistry(
      registry([solana], nativeTrustAnchor("solana-testnet")),
    ).lanes.length,
    1,
  );
});

test("registry requires one append-only native trust-anchor history and exact current pointer", () => {
  const route = governedRoute({ activation: "inbound_only" });
  const first = nativeTrustAnchor();
  const second = {
    ...structuredClone(first),
    anchor_hash: UPPER(0x92, 32),
    checkpoint_height: 2,
  };
  const canonical = registry([route], first);
  canonical.lanes[0].native_trust_anchors.push(second);
  canonical.lanes[0].current_native_trust_anchor_hash = second.anchor_hash;
  assert.equal(normalizeSccpRegistry(canonical).lanes.length, 1);

  const stalePointer = structuredClone(canonical);
  stalePointer.lanes[0].current_native_trust_anchor_hash = first.anchor_hash;
  assert.throws(() => normalizeSccpRegistry(stalePointer), /last retained anchor/u);

  const duplicate = structuredClone(canonical);
  duplicate.lanes[0].native_trust_anchors[1].anchor_hash = first.anchor_hash;
  duplicate.lanes[0].current_native_trust_anchor_hash = first.anchor_hash;
  assert.throws(() => normalizeSccpRegistry(duplicate), /duplicate native trust-anchor/u);

  const rollback = structuredClone(canonical);
  rollback.lanes[0].native_trust_anchors[1].checkpoint_height = 1;
  assert.throws(() => normalizeSccpRegistry(rollback), /advance monotonically/u);

  const legacy = structuredClone(canonical);
  legacy.lanes[0].native_trust_anchor = first;
  delete legacy.lanes[0].native_trust_anchors;
  delete legacy.lanes[0].current_native_trust_anchor_hash;
  assert.throws(() => normalizeSccpRegistry(legacy), /unknown or retired|field set/u);
});

test("retired routes require one complete retained-anchor finality interval", () => {
  const first = nativeTrustAnchor();
  const second = {
    ...structuredClone(first),
    anchor_hash: UPPER(0x92, 32),
    checkpoint_height: 2,
  };
  const cutoff = {
    trust_anchor_hash: first.anchor_hash,
    max_anchor_interval_height: second.checkpoint_height,
  };
  const canonical = registry(
    [governedRoute({ activation: "retired", inboundFinalityCutoff: cutoff })],
    first,
  );
  canonical.lanes[0].native_trust_anchors.push(second);
  canonical.lanes[0].current_native_trust_anchor_hash = second.anchor_hash;
  assert.equal(normalizeSccpRegistry(canonical).lanes.length, 1);

  const missing = structuredClone(canonical);
  missing.lanes[0].routes[0].inbound_finality_cutoff = null;
  assert.throws(() => normalizeSccpRegistry(missing), /required for a retired/u);

  const nonterminal = structuredClone(canonical);
  nonterminal.lanes[0].routes[0].activation.activation = "paused";
  assert.throws(() => normalizeSccpRegistry(nonterminal), /allowed only for a retired/u);

  for (const mutate of [
    (value) => {
      value.trust_anchor_hash = UPPER(0xff, 32);
    },
    (value) => {
      value.max_anchor_interval_height = second.checkpoint_height - 1;
    },
    (value) => {
      value.trust_anchor_hash = second.anchor_hash;
    },
  ]) {
    const incomplete = structuredClone(canonical);
    mutate(incomplete.lanes[0].routes[0].inbound_finality_cutoff);
    assert.throws(() => normalizeSccpRegistry(incomplete), /complete retained anchor interval/u);
  }

  const omitted = registry();
  delete omitted.lanes[0].routes[0].inbound_finality_cutoff;
  assert.throws(() => normalizeSccpRegistry(omitted), /field set|missing required/u);
});

test("registry accepts zero BN254 limbs but rejects an all-zero point", () => {
  const route = governedRoute();
  route.destination.deployment.verifying_key.alpha1.x = UPPER(0, 32);
  route.destination.deployment.verifier_key_hash = keyHash(
    route.destination.deployment.verifying_key,
  ).toUpperCase();
  route.source_identity.emitter.identity.route_config_hash =
    testDestinationHashes(route).routeConfigurationHash;
  assert.equal(normalizeSccpRegistry(registry([route])).lanes.length, 1);

  route.destination.deployment.verifying_key.alpha1.y = UPPER(0, 32);
  assert.throws(() => normalizeSccpRegistry(registry([route])), /point at infinity/u);
});

test("regenerated SCCP distribution enforces the canonical route commitments", async () => {
  const {
    normalizeSccpCapabilities: normalizeDistributionCapabilities,
    normalizeSccpProofRequest: normalizeDistributionProofRequest,
    normalizeSccpRecentMessages: normalizeDistributionRecentMessages,
    normalizeSccpRegistry: normalizeDistributionRegistry,
  } = await import("../dist/sccp.js");
  const { ToriiClient: DistributionToriiClient } = await import("../dist/toriiClient.js");
  assert.equal(
    normalizeDistributionCapabilities(capabilities()).resource_limits
      .max_pending_outbound_messages,
    65_536,
  );
  assert.deepEqual(
    normalizeDistributionRecentMessages({
      items: [recentItem(9, MESSAGE_ID, 7), recentItem(9, HASH(0x12), 8)],
      next: { from: 9, after_index: 8 },
    }).next,
    { from: 9, after_index: 8 },
  );
  const observedUrls = [];
  const distributionClient = new DistributionToriiClient("https://example.invalid", {
    fetchImpl: async (url) => {
      observedUrls.push(String(url));
      return response({ items: [] });
    },
  });
  assert.deepEqual(
    (await distributionClient.getSccpRecentMessages({
      from: 9,
      after_index: 8,
      limit: 1,
    })).items,
    [],
  );
  await assert.rejects(
    () => distributionClient.getSccpRecentMessages({ after_index: 0 }),
    /requires from/u,
  );
  assert.deepEqual(observedUrls, [
    "https://example.invalid/v1/sccp/messages/recent?from=9&after_index=8&limit=1",
  ]);
  assert.throws(
    () => normalizeDistributionProofRequest(crossPolicyAliasedProofRequest()),
    /proof-policy hash role/u,
  );
  for (const source of ["bsc-mainnet", "tron-mainnet"]) {
    assert.equal(
      normalizeDistributionRegistry(registry([governedRoute({ source })])).lanes.length,
      1,
    );
    for (const mutate of [
      (route) => {
        route.destination.deployment.outbound_proof_policy.semantic_profile.commitments
          .circuit_commitment = UPPER(0xc3, 32);
      },
      (route) => {
        route.destination.deployment.outbound_proof_policy.sora_finality_anchor
          .checkpoint_height += 1;
      },
      (route) => {
        route.destination.deployment.token_code_hash = UPPER(0x23, 32);
      },
    ]) {
      const route = governedRoute({ source });
      mutate(route);
      assert.throws(
        () => normalizeDistributionRegistry(registry([route])),
        /route_config_hash does not match/u,
      );
    }
  }
});

test("registry rejects retired families, browser metadata, duplicate lanes, and revision gaps", () => {
  for (const removed of ["sora_nexus", "sora-nexus"]) {
    const retiredSora = registry();
    retiredSora.lanes[0].lane_id.target = { network: removed, profile: null };
    assert.throws(() => normalizeSccpRegistry(retiredSora), /retired/u);
  }
  const retired = registry();
  retired.lanes[0].lane_id.source = { network: "solana_mainnet_beta", profile: null };
  assert.throws(() => normalizeSccpRegistry(retired), /retired/u);
  const browser = registry();
  browser.lanes[0].routes[0].destination_browser_prover = { module_url: "https://invalid" };
  assert.throws(() => normalizeSccpRegistry(browser), /unknown or retired/u);
  const duplicate = registry();
  duplicate.lanes.push(structuredClone(duplicate.lanes[0]));
  assert.throws(() => normalizeSccpRegistry(duplicate), /duplicate lane/u);
  const gap = registry([governedRoute({ revision: 2 })]);
  assert.throws(() => normalizeSccpRegistry(gap), /start at one/u);
  const doubleLive = registry([
    governedRoute({ revision: 1, activation: "bidirectional" }),
    governedRoute({ revision: 2, activation: "bidirectional" }),
  ], nativeTrustAnchor());
  assert.throws(() => normalizeSccpRegistry(doubleLive), /multiple revisions/u);
});

test("route governance accepts only closed atomic actions and exact field names", () => {
  const remove = {
    action: "Remove",
    route: {
      lane_id: lane(),
      route_id: "taira_bsc_xor",
      asset_key: "xor",
      revision: 1,
    },
  };
  assert.equal(normalizeSccpRouteGovernanceAction(remove).action, "Remove");
  assert.equal(
    normalizeSccpRouteGovernanceAction({
      action: "SetActivation",
      route: {
        key: remove.route,
        expected_current: { activation: "staged", direction: null },
        next: { activation: "inbound_only", direction: null },
        inbound_finality_cutoff: null,
      },
    }).action,
    "SetActivation",
  );
  const cutoff = {
    trust_anchor_hash: UPPER(0x91, 32),
    max_anchor_interval_height: 2,
  };
  assert.equal(
    normalizeSccpRouteGovernanceAction({
      action: "SetActivation",
      route: {
        key: remove.route,
        expected_current: { activation: "staged", direction: null },
        next: { activation: "retired", direction: null },
        inbound_finality_cutoff: cutoff,
      },
    }).action,
    "SetActivation",
  );
  assert.equal(
    normalizeSccpRouteGovernanceAction({
      action: "SwitchRevision",
      route: {
        previous_key: remove.route,
        expected_previous: { activation: "bidirectional", direction: null },
        previous_next: { activation: "retired", direction: null },
        previous_inbound_finality_cutoff: cutoff,
        successor_key: { ...remove.route, revision: 2 },
        successor_next: { activation: "bidirectional", direction: null },
      },
    }).action,
    "SwitchRevision",
  );
  for (const value of [
    { ...remove, manifest: {} },
    { action: "UpsertManifest", route: {} },
    { action: "Remove", route: { ...remove.route, routeId: "alias" } },
    {
      action: "SetActivation",
      route: {
        key: remove.route,
        expected_current: { activation: "staged", direction: null },
        next: { activation: "paused", direction: null },
        inbound_finality_cutoff: null,
      },
    },
    {
      action: "SetActivation",
      route: {
        key: remove.route,
        expected_current: { activation: "staged", direction: null },
        next: { activation: "retired", direction: null },
        inbound_finality_cutoff: null,
      },
    },
    {
      action: "SetActivation",
      route: {
        key: remove.route,
        expected_current: { activation: "staged", direction: null },
        next: { activation: "inbound_only", direction: null },
        inbound_finality_cutoff: cutoff,
      },
    },
  ]) assert.throws(() => normalizeSccpRouteGovernanceAction(value));
});

test("recent discovery validates compound commitment order, continuation, and exact links", () => {
  const parsed = normalizeSccpRecentMessages({
    items: [recentItem(9), recentItem(8, HASH(0x12))],
    next: { from: 8, after_index: 0 },
  });
  assert.deepEqual(parsed.items.map(({ height }) => height), [9, 8]);
  assert.deepEqual(parsed.next, { from: 8, after_index: 0 });
  const sameHeight = normalizeSccpRecentMessages({
    items: [
      recentItem(9, MESSAGE_ID, 509),
      recentItem(9, HASH(0x12), 510),
      recentItem(9, HASH(0x13), 511),
      recentItem(8, HASH(0x14), 0),
    ],
  });
  assert.deepEqual(sameHeight.items.map(({ commitment_index: index }) => index), [509, 510, 511, 0]);
  assert.equal(sameHeight.next, null);
  const fullRange = recentItem();
  fullRange.amount = "340282366920938463463374607431768211455";
  fullRange.payload_projection.Transfer.amount = fullRange.amount;
  fullRange.payload_projection.Transfer.nonce = "18446744073709551615";
  const fullRangeParsed = normalizeSccpRecentMessages({ items: [fullRange] });
  assert.equal(
    fullRangeParsed.items[0].payload_projection.Transfer.nonce,
    "18446744073709551615",
  );
  assert.equal(fullRangeParsed.items[0].payload_projection.Transfer.amount, fullRange.amount);
  const safeHeight = normalizeSccpRecentMessages({
    items: [recentItem(Number.MAX_SAFE_INTEGER)],
    next: { from: Number.MAX_SAFE_INTEGER, after_index: 0 },
  });
  assert.equal(safeHeight.items[0].height, Number.MAX_SAFE_INTEGER);
  const retired = recentItem();
  retired.links.artifact_path = `/v1/sccp/artifacts/message/${MESSAGE_ID}`;
  assert.throws(() => normalizeSccpRecentMessages({ items: [retired] }), /retired/u);
  const mismatch = recentItem();
  mismatch.links.proof_request_path = `/v1/sccp/proof-requests/${HASH(0x12)}`;
  assert.throws(() => normalizeSccpRecentMessages({ items: [mismatch] }), /exact message/u);
  const injection = recentItem();
  injection.links.bundle_path += "?allow_unready=true";
  assert.throws(() => normalizeSccpRecentMessages({ items: [injection] }));
  assert.throws(() => normalizeSccpRecentMessages({ items: [recentItem(8), recentItem(9)] }));
  assert.throws(() => normalizeSccpRecentMessages({ items: [recentItem(), recentItem()] }));
  const oversized = recentItem();
  oversized.amount = (1n << 128n).toString();
  assert.throws(() => normalizeSccpRecentMessages({ items: [oversized] }), /u128/u);
  for (const nonce of [7, "01", "+1", "18446744073709551616"]) {
    const invalid = recentItem();
    invalid.payload_projection.Transfer.nonce = nonce;
    assert.throws(() => normalizeSccpRecentMessages({ items: [invalid] }), /u64/u);
  }
  for (const amount of [1000, "0", "01", "+1", "340282366920938463463374607431768211456"]) {
    const invalid = recentItem();
    invalid.payload_projection.Transfer.amount = amount;
    assert.throws(() => normalizeSccpRecentMessages({ items: [invalid] }), /u128/u);
  }
  assert.throws(
    () => normalizeSccpRecentMessages({ items: Array.from({ length: 51 }, (_, index) => recentItem(51 - index, HASH(index + 1))) }),
    /50/u,
  );
  for (const mutate of [
    (value) => {
      delete value.payload_projection;
    },
    (value) => {
      value.payload_projection = null;
    },
    (value) => {
      value.payload_projection.Transfer.dest_domain = 5;
    },
    (value) => {
      value.payload_projection.Transfer.recipient = {
        CanonicalText: { value: "not-an-address" },
      };
    },
    (value) => {
      value.payload_projection.Transfer.route_id.CanonicalText.value = "taira_tron_xor";
    },
    (value) => {
      value.payload_projection.Transfer.amount = 0;
    },
    (value) => {
      value.amount = "1001";
    },
  ]) {
    const invalidProjection = recentItem();
    mutate(invalidProjection);
    assert.throws(() => normalizeSccpRecentMessages({ items: [invalidProjection] }));
  }
  for (const mutate of [
    (value) => { delete value.commitment_index; },
    (value) => { value.commitment_index = -1; },
    (value) => { value.commitment_index = 512; },
    (value) => { value.commitment_index = 1.5; },
  ]) {
    const invalidIndex = recentItem();
    mutate(invalidIndex);
    assert.throws(() => normalizeSccpRecentMessages({ items: [invalidIndex] }), /commitment_index/u);
  }
  const unsafeHeight = recentItem(Number.MAX_SAFE_INTEGER + 1);
  assert.throws(
    () => normalizeSccpRecentMessages({ items: [unsafeHeight] }),
    /safe integer/u,
  );
  for (const items of [
    [recentItem(9, MESSAGE_ID, 4), recentItem(9, HASH(0x12), 6)],
    [recentItem(9, MESSAGE_ID, 4), recentItem(9, HASH(0x12), 3)],
    [recentItem(9, MESSAGE_ID, 4), recentItem(8, HASH(0x12), 1)],
  ]) {
    assert.throws(() => normalizeSccpRecentMessages({ items }), /commitment index|indices/u);
  }
  for (const response of [
    { items: [recentItem(9)], next: null },
    { items: [], next: { from: 9, after_index: 0 } },
    { items: [recentItem(9, MESSAGE_ID, 3)], next: { from: 9, after_index: 2 } },
    { items: [recentItem(9, MESSAGE_ID, 3)], next: { from: 8, after_index: 3 } },
    { items: [recentItem(9, MESSAGE_ID, 3)], next: { from: 0, after_index: 3 } },
    { items: [recentItem(9, MESSAGE_ID, 3)], next: { from: 9, after_index: -1 } },
    { items: [recentItem(9, MESSAGE_ID, 3)], next: { from: 9, after_index: 1.5 } },
    { items: [recentItem(9, MESSAGE_ID, 3)], next: { from: 9, after_index: 512 } },
    { items: [recentItem(9, MESSAGE_ID, 3)], next: { from: 9, after_index: 3, cursor: 1 } },
  ]) {
    assert.throws(
      () => normalizeSccpRecentMessages(response),
      /continuation|after_index|unknown|plain object|safe integer/u,
    );
  }
});

test("recent discovery admits only the exact Solana projection shape", () => {
  const item = recentItem();
  item.target_profile = "solana-testnet";
  item.target_domain = SCCP_DOMAIN_SOLANA;
  item.route_id = "taira_sol_xor";
  item.payload_projection.Transfer.dest_domain = SCCP_DOMAIN_SOLANA;
  item.payload_projection.Transfer.recipient = {
    SolanaPubkey32: { bytes: `0x${"93".repeat(32)}` },
  };
  item.payload_projection.Transfer.route_id.CanonicalText.value = "taira_sol_xor";

  const parsed = normalizeSccpRecentMessages({ items: [item] });
  assert.equal(parsed.items[0].target_profile, "solana-testnet");
  assert.equal(
    parsed.items[0].payload_projection.Transfer.recipient.SolanaPubkey32.bytes,
    `0x${"93".repeat(32)}`,
  );

  item.payload_projection.Transfer.recipient.SolanaPubkey32.bytes = `0x${"00".repeat(32)}`;
  assert.throws(() => normalizeSccpRecentMessages({ items: [item] }), /nonzero/u);
});

test("bundle and proof-request JSON enforce the closed transfer/Groth16 schema", () => {
  assert.equal(normalizeSccpMessageBundle(messageBundle()).version, 1);
  assert.equal(normalizeSccpProofRequest(proofRequest()).public_inputs.target_domain, 2);
  const retiredPayload = messageBundle();
  retiredPayload.payload = { Burn: {} };
  assert.throws(() => normalizeSccpMessageBundle(retiredPayload), /retired/u);
  const aliasedCommitment = messageBundle();
  aliasedCommitment.commitment.context.route_configuration_hash =
    aliasedCommitment.commitment.context.destination_binding_hash;
  assert.throws(() => normalizeSccpMessageBundle(aliasedCommitment), /role-separated/u);
  const reservedDomain = messageBundle();
  reservedDomain.payload.Transfer.dest_domain = 4;
  assert.throws(() => normalizeSccpMessageBundle(reservedDomain), /reserved/u);
  const oversizedNonce = messageBundle();
  oversizedNonce.payload.Transfer.nonce = (1n << 64n).toString();
  assert.throws(() => normalizeSccpMessageBundle(oversizedNonce), /u64/u);
  const wrongRecipientCodec = messageBundle();
  wrongRecipientCodec.payload.Transfer.recipient_codec = 5;
  assert.throws(() => normalizeSccpMessageBundle(wrongRecipientCodec), /protocol domain/u);
  const longMerklePath = messageBundle();
  longMerklePath.merkle_proof.steps = Array.from({ length: 65 }, () => ({
    sibling_hash: PREFIX_HASH(0x70),
    sibling_is_left: false,
  }));
  assert.throws(() => normalizeSccpMessageBundle(longMerklePath), /64/u);
  const retiredBackend = proofRequest();
  retiredBackend.backend.backend = "solana_recursive_v1";
  assert.throws(() => normalizeSccpProofRequest(retiredBackend), /retired/u);
  const wrongFamily = proofRequest();
  wrongFamily.target_network = network("tron-mainnet");
  wrongFamily.public_inputs.target_domain = 5;
  assert.throws(() => normalizeSccpProofRequest(wrongFamily), /backend/u);
  const alias = proofRequest();
  alias.route_configuration_hash = alias.destination_binding_hash;
  assert.throws(() => normalizeSccpProofRequest(alias), /role-separated/u);
  const wrongKey = proofRequest();
  wrongKey.verifier_key_hash = PREFIX_HASH(0x99);
  assert.throws(() => normalizeSccpProofRequest(wrongKey), /does not match/u);
  const selector = proofRequest();
  selector.allow_unready = true;
  assert.throws(() => normalizeSccpProofRequest(selector), /retired/u);
  const wrongSemantic = proofRequest();
  wrongSemantic.semantic_proof_profile_hash = PREFIX_HASH(0x99);
  assert.throws(() => normalizeSccpProofRequest(wrongSemantic), /semantic_proof_profile_hash/u);
  const wrongAnchor = proofRequest();
  wrongAnchor.sora_finality_anchor_hash = PREFIX_HASH(0x99);
  assert.throws(() => normalizeSccpProofRequest(wrongAnchor), /sora_finality_anchor_hash/u);
  assert.throws(
    () => normalizeSccpProofRequest(crossPolicyAliasedProofRequest()),
    /proof-policy hash role/u,
  );
  const archivedIdentity = proofRequest();
  archivedIdentity.sora_finality_anchor.chain_id_hash = Buffer.from(
    keccak_256(Buffer.from("809574f5fee75e69bfcf52451e42d50f", "hex")),
  ).toString("hex").toUpperCase();
  assert.throws(() => normalizeSccpProofRequest(archivedIdentity), /Taira chain commitment/u);
});

test("submit DTOs preserve the exact prepared transaction for detached signing", () => {
  const transactionPayload = b64(Uint8Array.of(1, 2, 3, 4));
  const destinationProof = destinationProofB64();
  const nativeProof = nativeProofB64();
  const proof = normalizeBridgeProofSubmitPayload({
    authority: AUTHORITY,
    signature_b64: b64(new Uint8Array(64).fill(1)),
    transaction_payload_b64: transactionPayload,
    destination_proof_b64: destinationProof,
    creation_time_ms: 10,
  });
  assert.deepEqual(Object.keys(proof), [
    "authority",
    "signature_b64",
    "transaction_payload_b64",
    "destination_proof_b64",
    "creation_time_ms",
  ]);
  assert.equal(proof.transaction_payload_b64, transactionPayload);
  assert.deepEqual(Object.keys(normalizeBridgeMessageSubmitPayload({
    authority: AUTHORITY,
    native_proof_b64: nativeProof,
  })), ["authority", "native_proof_b64"]);
  const native = normalizeBridgeMessageSubmitPayload({
    authority: AUTHORITY,
    signature_b64: "AQ==",
    transaction_payload_b64: transactionPayload,
    native_proof_b64: nativeProof,
    creation_time_ms: 10,
  });
  assert.equal(native.transaction_payload_b64, transactionPayload);
});

test("submit DTOs reject mixed signing state, malformed encodings, and retired fields", () => {
  const proof = { authority: AUTHORITY, destination_proof_b64: destinationProofB64() };
  for (const [field, value] of [
    ["public_key_hex", HASH(1)],
    ["message_bundle_b64", "AQ=="],
    ["proof_bytes_hex", "01"],
    ["network_id_hex", HASH(2)],
    ["manifest_hash", HASH(3)],
    ["deployment", {}],
    ["allow_unready", true],
    ["signature", "AQ=="],
    ["client_signature_b64", "AQ=="],
  ]) assert.throws(() => normalizeBridgeProofSubmitPayload({ ...proof, [field]: value }));
  for (const artifact of ["AQ", " AQ==", "AQ==\n", "", "====", "A==="]) {
    assert.throws(() => normalizeBridgeProofSubmitPayload({ ...proof, destination_proof_b64: artifact }));
  }
  for (const signingState of [
    { signature_b64: "AQ==", creation_time_ms: 1 },
    { transaction_payload_b64: "AQ==", creation_time_ms: 1 },
    { signature_b64: "AQ==", transaction_payload_b64: "Ag==" },
    { signature_b64: "AQ", transaction_payload_b64: "Ag==", creation_time_ms: 1 },
    { signature_b64: "AQ==", transaction_payload_b64: "Ag", creation_time_ms: 1 },
  ]) {
    assert.throws(() => normalizeBridgeProofSubmitPayload({ ...proof, ...signingState }));
  }
  for (const creation_time_ms of [0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1, "1"]) {
    assert.throws(() => normalizeBridgeProofSubmitPayload({ ...proof, creation_time_ms }));
  }
});

test("submit DTOs bind the exact proof schema and require zero header padding", () => {
  assert.doesNotThrow(() => normalizeBridgeProofSubmitPayload({
    authority: AUTHORITY,
    destination_proof_b64: destinationProofB64(),
  }));
  assert.doesNotThrow(() => normalizeBridgeMessageSubmitPayload({
    authority: AUTHORITY,
    native_proof_b64: nativeProofB64(),
  }));
  assert.throws(() => normalizeBridgeProofSubmitPayload({
    authority: AUTHORITY,
    destination_proof_b64: nativeProofB64(),
  }), /schema hash/u);
  assert.throws(() => normalizeBridgeMessageSubmitPayload({
    authority: AUTHORITY,
    native_proof_b64: destinationProofB64(),
  }), /schema hash/u);
  for (const padding of [1, 8, 64]) {
    assert.throws(() => normalizeBridgeProofSubmitPayload({
      authority: AUTHORITY,
      destination_proof_b64: destinationProofB64({ padding }),
    }), /exactly 0 bytes/u);
    assert.throws(() => normalizeBridgeMessageSubmitPayload({
      authority: AUTHORITY,
      native_proof_b64: nativeProofB64({ padding }),
    }), /exactly 0 bytes/u);
  }
  assert.throws(() => normalizeBridgeProofSubmitPayload({
    authority: AUTHORITY,
    destination_proof_b64: destinationProofB64({ payload: Buffer.alloc(0) }),
  }), /non-empty/u);
  for (const authority of [ACCOUNT.toI105(753), ACCOUNT.toI105(0), ACCOUNT.toI105(370)]) {
    assert.throws(() => normalizeBridgeProofSubmitPayload({
      authority,
      destination_proof_b64: destinationProofB64(),
    }), /discriminant|prefix/u);
    assert.throws(() => normalizeBridgeMessageSubmitPayload({
      authority,
      native_proof_b64: nativeProofB64(),
    }), /discriminant|prefix/u);
  }
});

test("bridge response and JSON parser reject contradictions, aliases, and duplicate fields", () => {
  assert.equal(normalizeSccpBridgeSubmitResponse(preparedResponse()).submitted, false);
  assert.equal(normalizeSccpBridgeSubmitResponse({
    ...preparedResponse(),
    submitted: true,
    tx_hash_hex: HASH(0x55),
    transaction_payload_b64: null,
    signing_message_b64: null,
  }).submitted, true);
  for (const value of [
    { ...preparedResponse(), payload_kind: "burn" },
    { ...preparedResponse(), counterparty_chain: "solana-mainnet-beta" },
    { ...preparedResponse(), proof_artifact_hash: HASH(3) },
    { ...preparedResponse(), manifest_hash_hex: HASH(3) },
    { ...preparedResponse(), route_configuration_hash_hex: HASH(0xab).toUpperCase() },
    { ...preparedResponse(), creation_time_ms: 0 },
    { ...preparedResponse(), tx_hash_hex: HASH(4) },
    { ...preparedResponse(), transaction_payload_b64: b64(Uint8Array.of(1, 2, 3, 5)) },
    { ...preparedResponse(), signing_message_b64: b64(new Uint8Array(32).fill(9)) },
  ]) assert.throws(() => normalizeSccpBridgeSubmitResponse(value));
  const missingRouteHash = preparedResponse();
  delete missingRouteHash.route_configuration_hash_hex;
  assert.throws(() => normalizeSccpBridgeSubmitResponse(missingRouteHash), /missing required/u);
  assert.throws(
    () => normalizeSccpBridgeSubmitResponse(preparedResponse(), { submitted: true }),
    /signing state/u,
  );
  const json = JSON.stringify(preparedResponse());
  assert.equal(parseSccpBridgeSubmitResponseJson(json).submitted, false);
  assert.throws(() => parseSccpBridgeSubmitResponseJson(json.replace("{", '{"submitted":false,')), /duplicate/u);
  assert.throws(
    () => parseSccpBridgeSubmitResponseJson(
      json.replace(
        `"route_configuration_hash_hex":"${HASH(0x31)}"`,
        `"route_configuration_hash_hex":"${HASH(0x31)}","route_configuration_hash_hex":"${HASH(0x32)}"`,
      ),
    ),
    /duplicate/u,
  );
  assert.throws(() => parseSccpJsonObject(`${json}{}`), /trailing/u);
});

function response(
  value,
  {
    contentType = "application/json",
    contentLength,
    bytes,
    chunks: providedChunks,
    status = 200,
  } = {},
) {
  const bodyBytes = Buffer.from(bytes ?? Buffer.from(JSON.stringify(value), "utf8"));
  const chunks = (providedChunks ?? [bodyBytes]).map((chunk) => Uint8Array.from(chunk));
  const headers = new Headers({ "content-type": contentType });
  const streamState = { cancelled: false, released: false };
  let locked = false;
  let nextChunk = 0;
  const body = {
    get locked() { return locked; },
    getReader() {
      if (locked) throw new TypeError("test response body is already locked");
      locked = true;
      return {
        async read() {
          if (streamState.cancelled || nextChunk >= chunks.length) {
            return { done: true, value: undefined };
          }
          const value = chunks[nextChunk];
          nextChunk += 1;
          return { done: false, value };
        },
        async cancel() { streamState.cancelled = true; },
        releaseLock() { locked = false; streamState.released = true; },
      };
    },
    async cancel() { streamState.cancelled = true; },
  };
  return {
    status,
    statusText: status === 200 ? "OK" : "Test Error",
    headers: {
      get(name) {
        if (String(name).toLowerCase() === "content-length") {
          return contentLength ?? null;
        }
        return headers.get(name);
      },
    },
    body,
    streamState,
    async text() { return bodyBytes.toString("utf8"); },
    async arrayBuffer() {
      return bodyBytes.buffer.slice(
        bodyBytes.byteOffset,
        bodyBytes.byteOffset + bodyBytes.byteLength,
      );
    },
  };
}

function paddedJsonBytes(value, byteLength) {
  const canonical = Buffer.from(JSON.stringify(value), "utf8");
  assert.ok(canonical.length <= byteLength, "fixture must fit the requested byte length");
  return Buffer.concat([canonical, Buffer.alloc(byteLength - canonical.length, 0x20)]);
}

test("Torii exact client constructs fixed query-free endpoints and content negotiation", async () => {
  const proofRequestFrame = sccpNoritoFrame(PROOF_REQUEST_NORITO_TYPE);
  const observed = [];
  const client = new ToriiClient("https://example.invalid", {
    fetchImpl: async (url, init) => {
      observed.push({ url: String(url), accept: init.headers.Accept });
      const path = new URL(url).pathname;
      if (path === "/v1/sccp/capabilities") return response(capabilities());
      if (path === "/v1/sccp/registry") return response({ version: 1, lanes: [] });
      if (path.endsWith("/sora-outbound-material")) return response(soraOutboundMaterial());
      if (path.includes("proof-requests")) {
        return init.headers.Accept === "application/x-norito"
          ? response(null, { contentType: "application/x-norito", bytes: proofRequestFrame })
          : response(proofRequest());
      }
      if (path.includes("proofs/message")) return response(messageBundle());
      return response({ items: [] });
    },
  });
  assert.equal((await client.getSccpCapabilities()).version, 1);
  assert.equal((await client.getSccpRegistry()).version, 1);
  assert.equal(
    (
      await client.getSccpSoraOutboundMaterial({
        sourceProfile: "solana-testnet",
        routeId: "taira_sol_xor",
        assetKey: "xor",
        revision: 1,
      })
    ).policy.gas_limit,
    50_000_000,
  );
  assert.equal((await client.getSccpMessageBundle(MESSAGE_ID)).version, 1);
  assert.deepEqual(
    Buffer.from(await client.getSccpProofRequest(MESSAGE_ID, { format: "norito" })),
    proofRequestFrame,
  );
  assert.deepEqual(
    (await client.getSccpRecentMessages({ from: 9, after_index: 3, limit: 1 })).items,
    [],
  );
  assert.deepEqual(observed.map(({ url }) => url), [
    "https://example.invalid/v1/sccp/capabilities",
    "https://example.invalid/v1/sccp/registry",
    "https://example.invalid/v1/sccp/routes/solana-testnet/taira_sol_xor/xor/1/sora-outbound-material",
    `https://example.invalid/v1/sccp/proofs/message/${MESSAGE_ID}`,
    `https://example.invalid/v1/sccp/proof-requests/${MESSAGE_ID}`,
    "https://example.invalid/v1/sccp/messages/recent?from=9&after_index=3&limit=1",
  ]);
});

test("Torii SCCP Norito preflight accepts only the canonical zero-padding frame", async () => {
  const frame = sccpNoritoFrame(PROOF_REQUEST_NORITO_TYPE);
  const streamed = response(null, {
    contentType: "application/x-norito",
    bytes: frame,
  });
  const client = new ToriiClient("https://example.invalid", {
    fetchImpl: async () => streamed,
  });
  assert.deepEqual(
    Buffer.from(await client.getSccpProofRequest(MESSAGE_ID, { format: "norito" })),
    frame,
  );
  assert.equal(streamed.streamState.released, true);
});

test("Torii SCCP Norito preflight rejects malformed and cross-type frames", async () => {
  const canonical = sccpNoritoFrame(PROOF_REQUEST_NORITO_TYPE);
  const mutate = (offset, value) => {
    const frame = Buffer.from(canonical);
    frame[offset] = value;
    return frame;
  };
  const declaredLong = Buffer.from(canonical);
  declaredLong.writeBigUInt64LE(5n, 23);
  const declaredShort = Buffer.from(canonical);
  declaredShort.writeBigUInt64LE(3n, 23);
  const trailing = Buffer.concat([canonical, Buffer.from([0])]);
  const cases = [
    ["empty body", Buffer.alloc(0)],
    ["short header", canonical.subarray(0, 39)],
    ["magic", mutate(0, 0)],
    ["major version", mutate(4, 1)],
    ["minor version", mutate(5, 1)],
    ["zero schema", Buffer.concat([canonical.subarray(0, 6), Buffer.alloc(16), canonical.subarray(22)])],
    ["wrong response type", sccpNoritoFrame(MESSAGE_BUNDLE_NORITO_TYPE)],
    ["compressed payload", mutate(22, 1)],
    ["reserved flag", mutate(39, 0x08)],
    ["invalid bitset flags", mutate(39, 0x20)],
    ["declared payload too long", declaredLong],
    ["declared payload too short", declaredShort],
    ["checksum", mutate(31, canonical[31] ^ 0x01)],
    ["one-byte padding", sccpNoritoFrame(PROOF_REQUEST_NORITO_TYPE, { padding: 1 })],
    ["eight-byte padding", sccpNoritoFrame(PROOF_REQUEST_NORITO_TYPE, { padding: 8 })],
    ["64-byte padding", sccpNoritoFrame(PROOF_REQUEST_NORITO_TYPE, { padding: 64 })],
    ["65-byte padding", sccpNoritoFrame(PROOF_REQUEST_NORITO_TYPE, { padding: 65 })],
    ["trailing byte", trailing],
  ];
  for (const [label, bytes] of cases) {
    const malformed = response(null, {
      contentType: "application/x-norito",
      bytes,
    });
    const client = new ToriiClient("https://example.invalid", {
      fetchImpl: async () => malformed,
    });
    await assert.rejects(
      () => client.getSccpProofRequest(MESSAGE_ID, { format: "norito" }),
      undefined,
      label,
    );
    assert.equal(malformed.streamState.released, true, label);
  }
});

test("Torii SCCP streaming accepts an exact capability-size response", async () => {
  const maximumBytes = 64 * 1024;
  const exact = paddedJsonBytes(capabilities(), maximumBytes);
  const streamed = response(null, {
    bytes: exact,
    chunks: [exact.subarray(0, 7), exact.subarray(7, maximumBytes - 1), exact.subarray(maximumBytes - 1)],
    contentLength: String(maximumBytes),
  });
  const client = new ToriiClient("https://example.invalid", {
    fetchImpl: async () => streamed,
  });
  assert.equal((await client.getSccpCapabilities()).version, 1);
  assert.equal(streamed.streamState.cancelled, false);
  assert.equal(streamed.streamState.released, true);
});

test("Torii SCCP streaming rejects declared, missing-length, and understated overflows", async () => {
  const maximumBytes = 64 * 1024;
  const cases = [
    {
      name: "declared overflow",
      response: response(capabilities(), { contentLength: String(maximumBytes + 1) }),
    },
    {
      name: "actual overflow without Content-Length",
      response: response(null, { bytes: Buffer.alloc(maximumBytes + 1, 0x20) }),
    },
    {
      name: "actual overflow with understated Content-Length",
      response: response(null, {
        bytes: Buffer.alloc(maximumBytes + 1, 0x20),
        contentLength: "1",
      }),
    },
  ];
  for (const entry of cases) {
    const client = new ToriiClient("https://example.invalid", {
      fetchImpl: async () => entry.response,
    });
    await assert.rejects(
      () => client.getSccpCapabilities(),
      /65536-byte size bound/u,
      entry.name,
    );
    assert.equal(entry.response.streamState.cancelled, true, entry.name);
  }
});

test("Torii SCCP streaming rejects malformed and noncanonical Content-Length values", async () => {
  for (const contentLength of ["", "-1", "+1", "01", "1.0", "1, 1", "1 ", " 1"]) {
    const malformed = response(capabilities(), { contentLength });
    const client = new ToriiClient("https://example.invalid", {
      fetchImpl: async () => malformed,
    });
    await assert.rejects(
      () => client.getSccpCapabilities(),
      /Content-Length must be a canonical unsigned decimal integer/u,
      contentLength,
    );
    assert.equal(malformed.streamState.cancelled, true, contentLength);
  }
});

test("Torii SCCP streaming enforces strict UTF-8 before exact JSON parsing", async () => {
  const malformed = response(null, { bytes: Buffer.from([0x7b, 0x22, 0xff, 0x22, 0x7d]) });
  const client = new ToriiClient("https://example.invalid", {
    fetchImpl: async () => malformed,
  });
  await assert.rejects(() => client.getSccpCapabilities(), /strict UTF-8/u);
  assert.equal(malformed.streamState.released, true);
});

test("Torii SCCP error bodies are streamed through the same response bound", async () => {
  const oversizedError = response(null, {
    bytes: Buffer.alloc(64 * 1024 + 1, 0x20),
    status: 400,
  });
  const client = new ToriiClient("https://example.invalid", {
    fetchImpl: async () => oversizedError,
  });
  await assert.rejects(() => client.getSccpCapabilities(), /65536-byte size bound/u);
  assert.equal(oversizedError.streamState.cancelled, true);
});

test("Torii SCCP routes apply their endpoint-specific declared response limits", async () => {
  const cases = [
    {
      name: "recent JSON",
      maximumBytes: 8 * 1024 * 1024,
      invoke: (client) => client.getSccpRecentMessages(),
      contentType: "application/json",
    },
    {
      name: "native-bundle Norito",
      maximumBytes: 16 * 1024 * 1024,
      invoke: (client) => client.getSccpMessageBundle(MESSAGE_ID, { format: "norito" }),
      contentType: "application/x-norito",
    },
    {
      name: "destination-proof Norito",
      maximumBytes: 16 * 1024 * 1024 + 64 * 1024,
      invoke: (client) => client.getSccpProofRequest(MESSAGE_ID, { format: "norito" }),
      contentType: "application/x-norito",
    },
    {
      name: "submit JSON",
      maximumBytes: 64 * 1024 * 1024,
      invoke: (client) => client.submitBridgeProof({
        authority: AUTHORITY,
        destination_proof_b64: destinationProofB64(),
      }),
      contentType: "application/json",
    },
  ];
  for (const entry of cases) {
    const declaredOverflow = response({}, {
      contentLength: String(entry.maximumBytes + 1),
      contentType: entry.contentType,
    });
    const client = new ToriiClient("https://example.invalid", {
      fetchImpl: async () => declaredOverflow,
    });
    await assert.rejects(
      () => entry.invoke(client),
      new RegExp(`${entry.maximumBytes}-byte size bound`, "u"),
      entry.name,
    );
    assert.equal(declaredOverflow.streamState.cancelled, true, entry.name);
  }
});

test("Torii exact client rejects path/query injection and retired option aliases before fetch", async () => {
  let calls = 0;
  const client = new ToriiClient("https://example.invalid", {
    fetchImpl: async () => { calls += 1; return response({}); },
  });
  for (const id of [
    `0x${MESSAGE_ID}`,
    "ab".repeat(32).toUpperCase(),
    `${MESSAGE_ID}?network=bsc`,
    `${MESSAGE_ID}/../registry`,
    "00".repeat(32),
  ]) await assert.rejects(() => client.getSccpProofRequest(id));
  for (const options of [
    { network: "bsc-mainnet" },
    { allowUnready: true },
    { proofBytes: "01" },
    { format: "JSON" },
    { format: "artifact" },
  ]) await assert.rejects(() => client.getSccpProofRequest(MESSAGE_ID, options));
  for (const options of [
    { cursor: 1 },
    { from: 0 },
    { from: -1 },
    { from: "1" },
    { from: Number.MAX_SAFE_INTEGER + 1 },
    { after_index: 0 },
    { from: 1, after_index: -1 },
    { from: 1, after_index: 512 },
    { from: 1, after_index: 1.5 },
    { from: 1, after_index: "1" },
    { limit: 0 },
    { limit: -1 },
    { limit: 51 },
  ]) await assert.rejects(() => client.getSccpRecentMessages(options));
  for (const route of [
    { sourceProfile: "sora-taira", routeId: "taira_sol_xor", assetKey: "xor", revision: 1 },
    { sourceProfile: "solana-testnet", routeId: "taira_sol_xor/../registry", assetKey: "xor", revision: 1 },
    { sourceProfile: "solana-testnet", routeId: "taira_sol_xor", assetKey: "xor#universal", revision: 1 },
    { sourceProfile: "solana-testnet", routeId: "taira_sol_xor", assetKey: "xor", revision: 0 },
    { sourceProfile: "solana-testnet", routeId: "taira_sol_xor", assetKey: "xor", revision: 1, bytecode: "caller" },
  ]) await assert.rejects(() => client.getSccpSoraOutboundMaterial(route));
  assert.equal(calls, 0);
  assert.equal(typeof client.getSccpProofManifests, "undefined");
});

test("Torii proof submit sends only the closed destination artifact DTO", async () => {
  let observed;
  const client = new ToriiClient("https://example.invalid", {
    fetchImpl: async (url, init) => {
      observed = { url: String(url), body: JSON.parse(init.body) };
      return response(preparedResponse({ creation_time_ms: 42 }));
    },
  });
  await client.submitBridgeProof({
    authority: AUTHORITY,
    destination_proof_b64: destinationProofB64(),
    creation_time_ms: 42,
  });
  assert.deepEqual(observed, {
    url: "https://example.invalid/v1/bridge/proofs/submit",
    body: {
      authority: AUTHORITY,
      destination_proof_b64: destinationProofB64(),
      creation_time_ms: 42,
    },
  });
});

test("Torii prepare then submit resends the byte-identical transaction payload", async () => {
  const calls = [];
  const prepared = preparedResponse({ creation_time_ms: 42 });
  const client = new ToriiClient("https://example.invalid", {
    fetchImpl: async (url, init) => {
      const body = JSON.parse(init.body);
      calls.push({ url: String(url), body });
      if (calls.length === 1) return response(prepared);
      return response({
        ...prepared,
        submitted: true,
        tx_hash_hex: HASH(0x55),
        transaction_payload_b64: null,
        signing_message_b64: null,
      });
    },
  });
  const preparation = await client.submitBridgeProof({
    authority: AUTHORITY,
    destination_proof_b64: destinationProofB64(),
    creation_time_ms: 42,
  });
  const submission = await client.submitBridgeProof({
    authority: AUTHORITY,
    signature_b64: b64(new Uint8Array(64).fill(7)),
    transaction_payload_b64: preparation.transaction_payload_b64,
    destination_proof_b64: destinationProofB64(),
    creation_time_ms: preparation.creation_time_ms,
  });
  assert.equal(submission.submitted, true);
  assert.equal(calls[1].body.transaction_payload_b64, prepared.transaction_payload_b64);
  assert.deepEqual(
    [...Buffer.from(calls[1].body.transaction_payload_b64, "base64")],
    [1, 2, 3, 4],
  );
});

test("Torii rejects response state that contradicts prepare or signed submit", async () => {
  const submitted = {
    ...preparedResponse(),
    submitted: true,
    tx_hash_hex: HASH(0x55),
    transaction_payload_b64: null,
    signing_message_b64: null,
  };
  const prepareClient = new ToriiClient("https://example.invalid", {
    fetchImpl: async () => response(submitted),
  });
  await assert.rejects(
    () => prepareClient.submitBridgeProof({
      authority: AUTHORITY,
      destination_proof_b64: destinationProofB64(),
    }),
    /signing state/u,
  );
  const submitClient = new ToriiClient("https://example.invalid", {
    fetchImpl: async () => response(preparedResponse({ creation_time_ms: 42 })),
  });
  await assert.rejects(
    () => submitClient.submitBridgeProof({
      authority: AUTHORITY,
      signature_b64: "AQ==",
      transaction_payload_b64: "Ag==",
      destination_proof_b64: destinationProofB64(),
      creation_time_ms: 42,
    }),
    /signing state/u,
  );
});
