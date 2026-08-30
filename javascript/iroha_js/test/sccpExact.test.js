import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import test from "node:test";

import { keccak_256 } from "@noble/hashes/sha3";

import { AccountAddress } from "../src/address.js";
import { blake2b256 } from "../src/blake2b.js";
import * as sccpExports from "../src/sccp.js";
import {
  SCCP_CODEC_CANONICAL_TEXT,
  SCCP_CODEC_EVM_ADDRESS20,
  SCCP_CODEC_KEYS,
  SCCP_CODEC_TON_ACCOUNT36,
  SCCP_CODEC_TRON_ADDRESS21,
  SCCP_DOMAIN_TON,
  SCCP_NETWORK_PROFILES,
  SCCP_PAYLOAD_KINDS,
  deriveSccpTonDestinationHashesV1,
  normalizeSccpCapabilities,
  normalizeSccpCodecValue,
  normalizeSccpMessageBundle,
  normalizeSccpProofRequest,
  normalizeSccpRecentMessages,
  normalizeSccpRegistry,
  normalizeSccpRouteGovernanceAction,
  normalizeSccpSoraOutboundMaterial,
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
const TON_PROOF_REQUEST_NORITO_TYPE =
  "iroha_sccp::SccpTonGroth16Bls12381ProofRequestV1";
const PUBLIC_SIGNAL_SCHEMA_HASH =
  "7567439F41173D6745A3D51923CB70371ACC7D66F23CEFB4100D6D5D7A432CBB";
const SORA_TAIRA_CHAIN_ID_HASH =
  "CF1CFC0F57B0BFA4C21882A9870317A1F4812F86533897095E3944BE34C5BBA7";

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

function bls12381VerifyingKey() {
  const g1 = `80${"00".repeat(47)}`;
  const g2 = `${g1}${"00".repeat(48)}`;
  const ic = { constant: g1 };
  for (let index = 0; index < 11; index += 1) ic[`signal_${index}`] = g1;
  return { version: 1, alpha1: g1, beta2: g2, gamma2: g2, delta2: g2, ic };
}

function bls12381VerifyingKeyBytes(key) {
  return Buffer.concat([
    Buffer.from([1]),
    Buffer.from(key.alpha1, "hex"),
    Buffer.from(key.beta2, "hex"),
    Buffer.from(key.gamma2, "hex"),
    Buffer.from(key.delta2, "hex"),
    Buffer.from(key.ic.constant, "hex"),
    ...Array.from({ length: 11 }, (_, index) =>
      Buffer.from(key.ic[`signal_${index}`], "hex"),
    ),
  ]);
}

function bls12381KeyHash(key) {
  return createHash("sha256").update(bls12381VerifyingKeyBytes(key)).digest("hex");
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

function tonSemanticProfile() {
  return {
    profile: "sora_taira_finality_inclusion_groth16_bls12381",
    commitments: {
      version: 1,
      circuit_commitment: UPPER(0x95, 32),
      witness_generator_commitment: UPPER(0x96, 32),
      public_signal_schema_hash:
        "A4DB9F6AAC0ECD22AC107BFDAFBF30DD01087147517EFE285D345F3F1182B874",
    },
  };
}

function finalityAnchor(protocolVersion = 4) {
  return {
    version: 1,
    source_network: network("sora-taira"),
    protocol_version: protocolVersion,
    chain_id_hash: SORA_TAIRA_CHAIN_ID_HASH,
    checkpoint_height: 7,
    checkpoint_block_hash: UPPER(0xa1, 32),
    checkpoint_context_id: UPPER(0xa2, 32),
    checkpoint_finality_artifact_hash: UPPER(0xa3, 32),
  };
}

function outboundPolicy(protocolVersion = 4) {
  return {
    version: 1,
    semantic_profile: semanticProfile(),
    sora_finality_anchor: finalityAnchor(protocolVersion),
  };
}

function tonOutboundPolicy(protocolVersion = 4) {
  return {
    version: 1,
    semantic_profile: tonSemanticProfile(),
    sora_finality_anchor: finalityAnchor(protocolVersion),
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
        Buffer.from([
          1,
          semanticPolicy.profile ===
          "sora_taira_finality_inclusion_groth16_bls12381"
            ? 1
            : 0,
          1,
        ]),
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
        Buffer.from([1, 0x40]),
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

const BLS12381_SCALAR_MODULUS = BigInt(
  "0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001",
);
const TON_SIGNAL_LABELS = [
  "sccp:groth16-bls12381:signal:message-id:v1",
  "sccp:groth16-bls12381:signal:payload-hash:v1",
  "sccp:groth16-bls12381:signal:target-domain:v1",
  "sccp:groth16-bls12381:signal:commitment-root:v1",
  "sccp:groth16-bls12381:signal:finality-height:v1",
  "sccp:groth16-bls12381:signal:finality-block-hash:v1",
  "sccp:groth16-bls12381:signal:source-domain:v1",
  "sccp:groth16-bls12381:signal:statement-hash:v1",
  "sccp:groth16-bls12381:signal:destination-binding-hash:v1",
  "sccp:groth16-bls12381:signal:route-config-hash:v1",
  "sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1",
];
const TON_SIGNAL_FIELDS = [
  "message_id",
  "payload_hash",
  "target_domain",
  "commitment_root",
  "finality_height",
  "finality_block_hash",
  "source_domain",
  "statement_hash",
  "destination_binding_hash",
  "route_configuration_hash",
  "sora_finality_anchor_hash",
];

function tonProofProfileCommitment() {
  return createHash("sha256")
    .update(Buffer.from("sccp:ton:groth16-bls12381:proof-profile:v1"))
    .update(Buffer.from([1]))
    .update(Buffer.from("ietf-bls12381-compressed-g1-48-g2-96"))
    .update(Buffer.from("groth16-a-g1-b-g2-c-g1"))
    .update(Buffer.from("sha256-sha256-label-value-mod-r"))
    .update(Buffer.from(BLS12381_SCALAR_MODULUS.toString(16).padStart(64, "0"), "hex"))
    .update(Buffer.from(tonSemanticProfile().commitments.public_signal_schema_hash, "hex"))
    .digest();
}

function tonSignalWord(label, input) {
  const labelHash = createHash("sha256").update(label).digest();
  const digest = createHash("sha256").update(labelHash).update(input).digest();
  const scalar = BigInt(`0x${digest.toString("hex")}`) % BLS12381_SCALAR_MODULUS;
  return `0x${scalar.toString(16).padStart(64, "0")}`;
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

function signedLittleEndian32(value) {
  const result = Buffer.alloc(4);
  result.writeInt32LE(value);
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
  "sora-taira": Object.freeze({ tag: 0x40, domain: 0, bytes: Buffer.from("fc56984b2be7431d840e21514d1883f0", "hex") }),
  "ethereum-mainnet": Object.freeze({ tag: 0x41, domain: 1, bytes: littleEndian(1, 8), routeId: "taira_eth_xor", id: 1 }),
  "bsc-mainnet": Object.freeze({ tag: 0x42, domain: 2, bytes: littleEndian(56, 8), routeId: "taira_bsc_xor", id: 56 }),
  "tron-mainnet": Object.freeze({ tag: 0x43, domain: 3, bytes: littleEndian(0x2b66_53dc, 4), routeId: "taira_tron_xor", id: 0x2b66_53dc }),
  "ton-mainnet": Object.freeze({
    tag: 0x44,
    domain: 4,
    routeId: "taira_ton_xor",
    id: -239,
    bytes: concatenate(
      signedLittleEndian32(-239),
      signedLittleEndian32(-1),
      littleEndian(0x8000_0000_0000_0000n, 8),
      Buffer.alloc(4),
      Buffer.from("17a3a92992aabea785a7a090985a265cd31f323d849da51239737e321fb05569", "hex"),
      Buffer.from("5e994fcf4d425c0a6ce6a792594b7173205f740a39cd56f537defd28b48a0f6e", "hex"),
    ),
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
        addressWord(deployment.replay_verifier_address, tron),
        Buffer.from(deployment.replay_verifier_code_hash, "hex"),
        addressWord(deployment.mint_breaker_address, tron),
        Buffer.from(deployment.mint_breaker_code_hash, "hex"),
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
  deploymentWords.push(
    addressWord(deployment.replay_verifier_address),
    Buffer.from(deployment.replay_verifier_code_hash, "hex"),
    addressWord(deployment.mint_breaker_address),
    Buffer.from(deployment.mint_breaker_code_hash, "hex"),
  );
  const deploymentConfigHash = Buffer.from(keccak_256(concatenate(...deploymentWords)));
  const assetRouteConfigHash = Buffer.from(
    keccak_256(
      concatenate(
        keccak_256(Buffer.from("xor")),
        keccak_256(Buffer.from(descriptor.routeId)),
        abiWord(route.revision),
        abiWord(deployment.taira_to_token_multiplier),
        abiWord(deployment.max_wrapped_supply),
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
      max_ed25519_signature_checks_per_transaction: 65536,
      max_ed25519_signature_checks_per_block: 262144,
      max_ed25519_validator_key_checks_per_transaction: 198656,
      max_ed25519_validator_key_checks_per_block: 794624,
      max_bn254_pairing_checks_per_transaction: 1,
      max_bn254_pairing_checks_per_block: 4,
      max_bls12_381_pairing_checks_per_transaction: 1,
      max_bls12_381_pairing_checks_per_block: 4,
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
      lane_id: lane("bsc-mainnet"),
      route_id: "taira_bsc_xor",
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
        replay_verifier_address: UPPER(0x13, 20),
        replay_verifier_code_hash: UPPER(0x23, 32),
        mint_breaker_address: UPPER(0x14, 20),
        mint_breaker_code_hash: UPPER(0x24, 32),
        taira_to_token_multiplier: 1_000_000_000,
        max_wrapped_supply: "1000000000000000000000",
      },
    },
    sora_outbound_execution_policy: soraOutboundExecutionPolicy(),
    settlement: {
      asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      payload_amount_scale: 9,
      max_outstanding_liability: "1000000000000",
    },
  };
  route.source_identity.emitter.identity.route_config_hash =
    testDestinationHashes(route).routeConfigurationHash;
  return route;
}

function tonDeployment() {
  const key = bls12381VerifyingKey();
  return {
    jetton_master_address: { workchain: 0, account: UPPER(0x81, 32) },
    jetton_master_code_hash: UPPER(0x91, 32),
    jetton_master_initial_data_hash: UPPER(0x89, 32),
    jetton_wallet_code_hash: UPPER(0x92, 32),
    route_address: { workchain: 0, account: UPPER(0x82, 32) },
    route_code_hash: UPPER(0x93, 32),
    route_initial_data_hash: UPPER(0x8a, 32),
    embedded_verifier_code_hash: UPPER(0x94, 32),
    verifier_circuit_hash: UPPER(0x95, 32),
    verifying_key: key,
    verifier_key_hash: bls12381KeyHash(key).toUpperCase(),
    proof_profile_commitment: tonProofProfileCommitment().toString("hex").toUpperCase(),
    mint_breaker_guardian_keys: {
      guardian_0: UPPER(0xa1, 32),
      guardian_1: UPPER(0xa2, 32),
      guardian_2: UPPER(0xa3, 32),
      guardian_3: UPPER(0xa4, 32),
      guardian_4: UPPER(0xa5, 32),
    },
    outbound_proof_policy: tonOutboundPolicy(),
    taira_to_token_multiplier: 1,
    max_wrapped_supply: "1000000000000",
  };
}

function tonGovernedRoute({ source = "ton-mainnet", activation = "staged" } = {}) {
  const deployment = tonDeployment();
  const routeHashes = deriveSccpTonDestinationHashesV1(deployment, source, 1);
  return {
    lane_id: lane(source),
    route_id: "taira_ton_xor",
    asset_key: "xor",
    revision: 1,
    activation: { activation, direction: null },
    inbound_finality_cutoff: null,
    source_identity: {
      lane: lane(source),
      emitter: {
        emitter: "ton",
        identity: {
          address: structuredClone(deployment.route_address),
          code_hash: deployment.route_code_hash,
          route_config_hash: routeHashes.route_configuration_hash.slice(2).toUpperCase(),
        },
      },
    },
    destination: { family: "ton", deployment },
    sora_outbound_execution_policy: soraOutboundExecutionPolicy(),
    settlement: {
      asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      payload_amount_scale: 9,
      max_outstanding_liability: "1000000000000",
    },
  };
}

function nativeTrustAnchor(source = "bsc-mainnet") {
  const backend = source.startsWith("tron-")
    ? "tron_dpos_v1"
    : source.startsWith("ton-")
      ? "ton_masterchain_v1"
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
        asset_id_codec: 0,
        asset_id: "0x786f72",
        amount: "1",
        sender_codec: 0,
        sender: "0x616c696365",
        recipient_codec: 1,
        recipient: `0x${HASH(0x21).slice(0, 40)}`,
        route_id_codec: 0,
        route_id: "0x74616972615f6273635f786f72",
      },
    },
    finality_proof: "0x0102",
  };
}

function proofRequest(protocolVersion = 4) {
  const key = verifyingKey();
  const policy = outboundPolicy(protocolVersion);
  const hashes = policyHashes(policy);
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

function tonProofRequest(protocolVersion = 4) {
  const key = bls12381VerifyingKey();
  const policy = tonOutboundPolicy(protocolVersion);
  const hashes = policyHashes(policy);
  const request = {
    version: 1,
    backend: { backend: "ton_groth16_bls12381_v1", family: null },
    source_network: network("sora-taira"),
    target_network: network("ton-mainnet"),
    public_inputs: {
      version: 1,
      message_id: PREFIX_HASH(0x11),
      payload_hash: PREFIX_HASH(0x12),
      target_domain: 4,
      commitment_root: PREFIX_HASH(0x13),
      finality_height: "9",
      finality_block_hash: PREFIX_HASH(0x14),
    },
    public_signals: {},
    verifying_key: key,
    verifier_key_hash: `0x${bls12381KeyHash(key)}`,
    verifier_circuit_hash: `0x${policy.semantic_profile.commitments.circuit_commitment.toLowerCase()}`,
    proof_profile_commitment: `0x${tonProofProfileCommitment().toString("hex")}`,
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
  const inputWords = [
    Buffer.from(request.public_inputs.message_id.slice(2), "hex"),
    Buffer.from(request.public_inputs.payload_hash.slice(2), "hex"),
    abiWord(4),
    Buffer.from(request.public_inputs.commitment_root.slice(2), "hex"),
    abiWord(9),
    Buffer.from(request.public_inputs.finality_block_hash.slice(2), "hex"),
    abiWord(0),
    Buffer.from(request.statement_hash.slice(2), "hex"),
    Buffer.from(request.destination_binding_hash.slice(2), "hex"),
    Buffer.from(request.route_configuration_hash.slice(2), "hex"),
    Buffer.from(request.sora_finality_anchor_hash.slice(2), "hex"),
  ];
  TON_SIGNAL_FIELDS.forEach((field, index) => {
    request.public_signals[field] = tonSignalWord(TON_SIGNAL_LABELS[index], inputWords[index]);
  });
  return request;
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

test("closed SCCP inventory exposes only the four external mainnets and Sora Taira", async () => {
  assert.deepEqual(Object.keys(SCCP_NETWORK_PROFILES), [
    "sora-taira",
    "ethereum-mainnet",
    "bsc-mainnet",
    "tron-mainnet",
    "ton-mainnet",
  ]);
  assert.equal(Object.values(SCCP_NETWORK_PROFILES).some(({ tag }) => tag === 0), false);
  assert.deepEqual(
    Object.values(SCCP_NETWORK_PROFILES).map(({ tag }) => tag),
    [0x40, 0x41, 0x42, 0x43, 0x44],
  );
  const declarations = fs.readFileSync(new URL("../index.d.ts", import.meta.url), "utf8");
  assert.match(
    declarations,
    /export type SccpNetworkTag = 0x40 \| 0x41 \| 0x42 \| 0x43 \| 0x44;/u,
  );
  assert.match(
    declarations,
    /Tag extends SccpNetworkTag = SccpNetworkTag,[^}]*readonly tag: Tag;/u,
  );
  for (const [profile, tag, domain, sora] of [
    ["sora-taira", "0x40", 0, true],
    ["ethereum-mainnet", "0x41", 1, false],
    ["bsc-mainnet", "0x42", 2, false],
    ["tron-mainnet", "0x43", 3, false],
    ["ton-mainnet", "0x44", 4, false],
  ]) {
    assert.match(
      declarations,
      new RegExp(
        `readonly "${profile}": SccpNetworkDescriptor<"${profile}", ${tag}, ${domain}, ${sora}>`,
        "u",
      ),
    );
  }
  assert.doesNotMatch(declarations, /SccpNetworkDescriptor \{[^}]*readonly tag: number;/u);
  for (const descriptor of Object.values(SCCP_NETWORK_PROFILES)) {
    assert.equal("genesisHash" in descriptor, false);
  }
  assert.deepEqual(Object.keys(SCCP_CODEC_KEYS), ["0", "1", "2", "3"]);
  assert.deepEqual(SCCP_PAYLOAD_KINDS, ["transfer"]);
  assert.deepEqual(SCCP_NETWORK_PROFILES["ton-mainnet"], {
    profile: "ton-mainnet",
    tag: 0x44,
    domain: SCCP_DOMAIN_TON,
    sora: false,
    globalId: -239,
  });
  const exports = await import("../src/sccp.js");
  for (const retired of [
    "SCCP_DOMAIN_SOL",
    "SCCP_DOMAIN_SOLANA",
    "SCCP_CODEC_SOLANA_PUBKEY32",
    "SCCP_CODEC_SOLANA_BASE58",
    "SCCP_SOLANA_TESTNET_GENESIS_HASH",
    "deriveSccpSolanaDestinationHashesV1",
    "deriveSccpSolanaNativeVerifierConfigHashV1",
    "deriveSccpSolanaSourceIdentityHashesV1",
    "SCCP_CODEC_SORA_ASSET_ID",
    "normalizeSccpProofManifests",
    "normalizeSccpSourceAdapterEngineDeployment",
  ]) {
    assert.equal(retired in exports, false, retired);
  }
});

test("closed codecs accept exact layouts and reject retired tags and textual aliases", () => {
  assert.deepEqual(normalizeSccpCodecValue(0, "merchant@taira"), new TextEncoder().encode("merchant@taira"));
  assert.match(AUTHORITY, /[^\x00-\x7f]/u, "fixture must exercise non-ASCII I105 digits");
  assert.deepEqual(normalizeSccpCodecValue(0, AUTHORITY), new TextEncoder().encode(AUTHORITY));
  assert.equal(normalizeSccpCodecValue(1, new Uint8Array(20).fill(1)).length, 20);
  assert.equal(
    normalizeSccpCodecValue(2, Uint8Array.from([0x41, ...new Uint8Array(20).fill(2)])).length,
    21,
  );
  assert.equal(
    normalizeSccpCodecValue(
      SCCP_CODEC_TON_ACCOUNT36,
      Uint8Array.from([...new Uint8Array(4), ...new Uint8Array(32).fill(4)]),
    ).length,
    36,
  );
  for (const [tag, value] of [
    [3, new Uint8Array(32).fill(1)],
    [4, new Uint8Array(36).fill(1)],
    [6, Uint8Array.of(1)],
    [6, new Uint8Array(32)],
    [6, "11111111111111111111111111111111"],
    [7, new Uint8Array(36)],
    [7, Uint8Array.from([0, 0, 0, 1, ...new Uint8Array(32).fill(1)])],
    [7, new Uint8Array(35).fill(1)],
    [2, `0x${"11".repeat(20)}`],
    [2, new Uint8Array(20)],
    [2, Uint8Array.from([0x42, ...new Uint8Array(20).fill(1)])],
    [0, " padded"],
    [0, "contains space"],
    [0, "line\nbreak"],
    [0, "merchant\ud83d\ude42"],
    [0, `${AUTHORITY.slice(0, -1)}${AUTHORITY.endsWith("1") ? "2" : "1"}`],
    [0, `n369${AUTHORITY.slice("test".length)}`],
    [0, `${AUTHORITY}${"\uff72".repeat(100)}`],
  ]) assert.throws(() => normalizeSccpCodecValue(tag, value));
});

test("source-event digest matches all shared ETH/BSC/TRON/TON mainnet vectors", () => {
  const fixture = JSON.parse(
    fs.readFileSync(new URL("../../../fixtures/sccp/native_transfer_event_v1.json", import.meta.url), "utf8"),
  );
  assert.deepEqual(
    fixture.vectors.map(({ source_profile }) => source_profile),
    ["ethereum-mainnet", "bsc-mainnet", "tron-mainnet", "ton-mainnet"],
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
    sourceProfile: "bsc-mainnet",
    routeId: "taira_bsc_xor",
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
    (value) => { value.route_key.route_id = "legacy_bsc_xor"; },
    (value) => { value.route_key.legacy_route = "taira_bsc_xor"; },
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
          sourceProfile: "bsc-mainnet",
          routeId: "taira_bsc_xor",
          assetKey: "xor",
          revision: 1,
        }),
    );
  }
  assert.throws(
    () => normalizeSccpSoraOutboundMaterial(material, { routeId: "alias_bsc_xor" }),
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
    ["max_ed25519_signature_checks_per_transaction", "max_ed25519_signature_checks_per_block", /transaction resource limits/u],
    ["max_ed25519_validator_key_checks_per_transaction", "max_ed25519_validator_key_checks_per_block", /transaction resource limits/u],
    ["max_bn254_pairing_checks_per_transaction", "max_bn254_pairing_checks_per_block", /transaction resource limits/u],
    ["max_bls12_381_pairing_checks_per_transaction", "max_bls12_381_pairing_checks_per_block", /transaction resource limits/u],
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
      destinationBindingHash: "68A718F971BBDEEA456B325B7821E20B6CBDE82A1C5FB520D31E0D27F0B2D452",
      deploymentConfigHash: "BC7ECD599C20CECACE8B28139EB6949C9BF490E2BF04F06C12D59A3BEFB38C8C",
      routeConfigurationHash: "4776F5FBE731E2EEBD827BAF080DB67ABE1A1E8F78F79A1B741CB004A2A992AD",
    },
    {
      source: "tron-mainnet",
      destinationBindingHash: "83B2BB7F5497E89D613DF3C6CFB745D84C4976DD4B447DDAE65D8B485EE9A408",
      deploymentConfigHash: "F95AD7752CF34AA4BF813E23CF517591372DBB7FEF6E344C37ED16BE63FF3414",
      routeConfigurationHash: "060705F1FB6C32BDE115DD29B6885CADE7F734DF4768772F1DA857C914018FD9",
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

test("TON registry enforces account, storage, BLS12-381, and fixed-point-safe hash roles", () => {
  const deployment = tonDeployment();
  const hashes = deriveSccpTonDestinationHashesV1(deployment, "ton-mainnet", 1);
  for (const value of Object.values(hashes)) assert.match(value, /^0x[0-9a-f]{64}$/u);
  assert.equal(normalizeSccpRegistry(registry([tonGovernedRoute()])).lanes.length, 1);

  for (const field of ["jetton_master_initial_data_hash", "route_initial_data_hash"]) {
    const changed = structuredClone(deployment);
    changed[field] = field === "jetton_master_initial_data_hash" ? UPPER(0x87, 32) : UPPER(0x88, 32);
    assert.deepEqual(
      deriveSccpTonDestinationHashesV1(changed, "ton-mainnet", 1),
      hashes,
      `${field} must stay outside fixed-point D/R preimages`,
    );
  }
  for (const field of ["jetton_master_address", "route_address"]) {
    const changed = structuredClone(deployment);
    changed[field].account = field === "jetton_master_address" ? UPPER(0x84, 32) : UPPER(0x85, 32);
    assert.deepEqual(
      deriveSccpTonDestinationHashesV1(changed, "ton-mainnet", 1),
      hashes,
      `${field} must stay outside fixed-point D/R preimages`,
    );
  }
  const changedCode = structuredClone(deployment);
  changedCode.jetton_master_code_hash = UPPER(0x86, 32);
  assert.notDeepEqual(
    deriveSccpTonDestinationHashesV1(changedCode, "ton-mainnet", 1),
    hashes,
  );
  const aliasedStorage = structuredClone(deployment);
  aliasedStorage.route_initial_data_hash = aliasedStorage.jetton_master_initial_data_hash;
  assert.throws(
    () => deriveSccpTonDestinationHashesV1(aliasedStorage, "ton-mainnet", 1),
    /reuses/u,
  );
  const wrongWorkchain = structuredClone(deployment);
  wrongWorkchain.route_address.workchain = -1;
  assert.throws(
    () => deriveSccpTonDestinationHashesV1(wrongWorkchain, "ton-mainnet", 1),
    /basechain/u,
  );
  const sourceAlias = tonGovernedRoute();
  sourceAlias.source_identity.emitter.identity.address = structuredClone(
    sourceAlias.destination.deployment.jetton_master_address,
  );
  assert.throws(
    () => normalizeSccpRegistry(registry([sourceAlias])),
    /source emitter does not identify/u,
  );
  const zeroGuardian = structuredClone(deployment);
  zeroGuardian.mint_breaker_guardian_keys.guardian_2 = UPPER(0, 32);
  assert.throws(
    () => deriveSccpTonDestinationHashesV1(zeroGuardian, "ton-mainnet", 1),
    /nonzero/u,
  );
  const unsortedGuardians = structuredClone(deployment);
  unsortedGuardians.mint_breaker_guardian_keys.guardian_3 =
    unsortedGuardians.mint_breaker_guardian_keys.guardian_2;
  assert.throws(
    () => deriveSccpTonDestinationHashesV1(unsortedGuardians, "ton-mainnet", 1),
    /strictly increasing/u,
  );
  const zeroCap = structuredClone(deployment);
  zeroCap.max_wrapped_supply = "0";
  assert.throws(
    () => deriveSccpTonDestinationHashesV1(zeroCap, "ton-mainnet", 1),
    /canonical positive/u,
  );
  const oversizedCap = structuredClone(deployment);
  oversizedCap.max_wrapped_supply = (1n << 120n).toString();
  assert.throws(
    () => deriveSccpTonDestinationHashesV1(oversizedCap, "ton-mainnet", 1),
    /positive u128/u,
  );
  const aliasedExecutionArtifact = tonGovernedRoute();
  aliasedExecutionArtifact.sora_outbound_execution_policy.contract_artifact_sha256 =
    aliasedExecutionArtifact.destination.deployment.route_initial_data_hash;
  assert.throws(
    () => normalizeSccpRegistry(registry([aliasedExecutionArtifact])),
    /reuses/u,
  );
  const mismatchedCircuit = structuredClone(deployment);
  mismatchedCircuit.verifier_circuit_hash = UPPER(0x96, 32);
  assert.throws(
    () => deriveSccpTonDestinationHashesV1(mismatchedCircuit, "ton-mainnet", 1),
    /circuit/u,
  );
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
    ["future protocol", (anchor) => { anchor.protocol_version = 5; }, /protocol_version/u],
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
    ["token address", (route) => { route.destination.deployment.token_address = UPPER(0x15, 20); }],
    ["token code", (route) => { route.destination.deployment.token_code_hash = UPPER(0x25, 32); }],
    ["verifier address", (route) => { route.destination.deployment.verifier_address = UPPER(0x16, 20); }],
    ["verifier code", (route) => { route.destination.deployment.verifier_code_hash = UPPER(0x26, 32); }],
    ["replay verifier address", (route) => { route.destination.deployment.replay_verifier_address = UPPER(0x17, 20); }],
    ["replay verifier code", (route) => { route.destination.deployment.replay_verifier_code_hash = UPPER(0x27, 32); }],
    ["mint breaker address", (route) => { route.destination.deployment.mint_breaker_address = UPPER(0x18, 20); }],
    ["mint breaker code", (route) => { route.destination.deployment.mint_breaker_code_hash = UPPER(0x28, 32); }],
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

test("EVM and TRON destination bindings commit replay-verifier and mint-breaker roles", () => {
  const mutations = [
    ["replay verifier address", (route) => { route.destination.deployment.replay_verifier_address = UPPER(0x17, 20); }],
    ["replay verifier code", (route) => { route.destination.deployment.replay_verifier_code_hash = UPPER(0x27, 32); }],
    ["mint breaker address", (route) => { route.destination.deployment.mint_breaker_address = UPPER(0x18, 20); }],
    ["mint breaker code", (route) => { route.destination.deployment.mint_breaker_code_hash = UPPER(0x28, 32); }],
  ];
  for (const source of ["bsc-mainnet", "tron-mainnet"]) {
    const baseline = testDestinationHashes(governedRoute({ source }));
    for (const [label, mutate] of mutations) {
      const route = governedRoute({ source });
      mutate(route);
      const changed = testDestinationHashes(route);
      assert.notEqual(changed.destinationBindingHash, baseline.destinationBindingHash, label);
      route.source_identity.emitter.identity.route_config_hash = changed.routeConfigurationHash;
      route.sora_outbound_execution_policy.contract_artifact_sha256 =
        changed.destinationBindingHash;
      assert.throws(
        () => normalizeSccpRegistry(registry([route])),
        /sora_outbound_execution_policy.*reuses/u,
        `${source} ${label}`,
      );
    }
  }
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
});

test("registry rejects every removed external profile", () => {
  for (const source of [
    "ethereum-sepolia",
    "bsc-testnet",
    "tron-nile",
    "tron-shasta",
    "solana-mainnet-beta",
    "solana-testnet",
    "ton-testnet",
  ]) {
    const value = registry();
    value.lanes[0].lane_id.source = {
      network: source.replaceAll("-", "_"),
      profile: null,
    };
    assert.throws(() => normalizeSccpRegistry(value), /retired network|unsupported/u);
  }
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
        route.destination.deployment.token_code_hash = UPPER(0x7e, 32);
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
  const retiredCustody = registry();
  retiredCustody.lanes[0].routes[0].settlement.custody_owner = AUTHORITY;
  assert.throws(() => normalizeSccpRegistry(retiredCustody), /unknown or retired/u);
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

test("recent discovery admits only canonical TON basechain projections", () => {
  const item = recentItem();
  item.target_profile = "ton-mainnet";
  item.target_domain = 4;
  item.route_id = "taira_ton_xor";
  item.payload_projection.Transfer.dest_domain = 4;
  item.payload_projection.Transfer.route_id.CanonicalText.value = "taira_ton_xor";
  item.payload_projection.Transfer.recipient = {
    TonAccount36: { workchain: 0, account: `0x${"93".repeat(32)}` },
  };
  const parsed = normalizeSccpRecentMessages({ items: [item] });
  assert.equal(parsed.items[0].payload_projection.Transfer.recipient.TonAccount36.workchain, 0);

  item.payload_projection.Transfer.recipient.TonAccount36.workchain = -1;
  assert.throws(() => normalizeSccpRecentMessages({ items: [item] }), /workchain/u);
  item.payload_projection.Transfer.recipient.TonAccount36.workchain = 0;
  item.payload_projection.Transfer.recipient.TonAccount36.account = `0x${"00".repeat(32)}`;
  assert.throws(() => normalizeSccpRecentMessages({ items: [item] }), /nonzero/u);
});

test("TON proof requests authenticate exact BLS12-381 keys, profiles, and signals", () => {
  const parsed = normalizeSccpProofRequest(tonProofRequest());
  assert.equal(parsed.backend.backend, "ton_groth16_bls12381_v1");
  assert.equal(parsed.public_inputs.target_domain, SCCP_DOMAIN_TON);

  const wrongSignal = tonProofRequest();
  wrongSignal.public_signals.message_id = PREFIX_HASH(0x99);
  assert.throws(() => normalizeSccpProofRequest(wrongSignal), /exact request role/u);
  const uncompressed = tonProofRequest();
  uncompressed.verifying_key.alpha1 = `00${"00".repeat(47)}`;
  assert.throws(() => normalizeSccpProofRequest(uncompressed), /nonzero|compressed BLS12-381/u);
  const wrongProfile = tonProofRequest();
  wrongProfile.semantic_proof_profile = semanticProfile();
  assert.throws(() => normalizeSccpProofRequest(wrongProfile), /destination backend/u);
  const bnWithTonField = proofRequest();
  bnWithTonField.public_signals = {};
  assert.throws(() => normalizeSccpProofRequest(bnWithTonField), /retired field/u);
});

test("bundle and proof-request JSON enforce the closed transfer/Groth16 schema", () => {
  assert.equal(normalizeSccpMessageBundle(messageBundle()).version, 1);
  const request = normalizeSccpProofRequest(proofRequest());
  assert.equal(request.public_inputs.target_domain, 2);
  assert.equal(request.sora_finality_anchor.protocol_version, 4);
  assert.throws(
    () => normalizeSccpProofRequest(proofRequest(3)),
    /protocol_version/u,
  );
  const retiredPayload = messageBundle();
  retiredPayload.payload = { Burn: {} };
  assert.throws(() => normalizeSccpMessageBundle(retiredPayload), /retired/u);
  const aliasedCommitment = messageBundle();
  aliasedCommitment.commitment.context.route_configuration_hash =
    aliasedCommitment.commitment.context.destination_binding_hash;
  assert.throws(() => normalizeSccpMessageBundle(aliasedCommitment), /role-separated/u);
  const mismatchedTonDomain = messageBundle();
  mismatchedTonDomain.payload.Transfer.dest_domain = 4;
  assert.throws(() => normalizeSccpMessageBundle(mismatchedTonDomain), /exact lane/u);
  const oversizedNonce = messageBundle();
  oversizedNonce.payload.Transfer.nonce = (1n << 64n).toString();
  assert.throws(() => normalizeSccpMessageBundle(oversizedNonce), /u64/u);
  const wrongRecipientCodec = messageBundle();
  wrongRecipientCodec.payload.Transfer.recipient_codec = 0;
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
        sourceProfile: "bsc-mainnet",
        routeId: "taira_bsc_xor",
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
    "https://example.invalid/v1/sccp/routes/bsc-mainnet/taira_bsc_xor/xor/1/sora-outbound-material",
    `https://example.invalid/v1/sccp/proofs/message/${MESSAGE_ID}`,
    `https://example.invalid/v1/sccp/proof-requests/${MESSAGE_ID}`,
    "https://example.invalid/v1/sccp/messages/recent?from=9&after_index=3&limit=1",
  ]);
});

test("Torii SCCP Norito preflight accepts both concrete request types with zero padding", async () => {
  for (const typeName of [PROOF_REQUEST_NORITO_TYPE, TON_PROOF_REQUEST_NORITO_TYPE]) {
    const frame = sccpNoritoFrame(typeName);
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
  }
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
    ["unknown response type", sccpNoritoFrame("example::UnknownProofRequestV1")],
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
    { sourceProfile: "sora-taira", routeId: "taira_bsc_xor", assetKey: "xor", revision: 1 },
    { sourceProfile: "bsc-mainnet", routeId: "taira_bsc_xor/../registry", assetKey: "xor", revision: 1 },
    { sourceProfile: "bsc-mainnet", routeId: "taira_bsc_xor", assetKey: "xor#universal", revision: 1 },
    { sourceProfile: "bsc-mainnet", routeId: "taira_bsc_xor", assetKey: "xor", revision: 0 },
    { sourceProfile: "bsc-mainnet", routeId: "taira_bsc_xor", assetKey: "xor", revision: 1, bytecode: "caller" },
  ]) await assert.rejects(() => client.getSccpSoraOutboundMaterial(route));
  assert.equal(calls, 0);
  assert.equal(typeof client.getSccpProofManifests, "undefined");
});

test("first-release JS SCCP exports no unauthenticated write surface", () => {
  for (const retired of [
    "normalizeBridgeProofSubmitPayload",
    "normalizeBridgeMessageSubmitPayload",
    "normalizeSccpBridgeSubmitResponse",
    "parseSccpBridgeSubmitResponseJson",
  ]) {
    assert.equal(retired in sccpExports, false, retired);
  }
  assert.equal(typeof ToriiClient.prototype.submitBridgeProof, "undefined");
  assert.equal(typeof ToriiClient.prototype.submitBridgeMessage, "undefined");

  const declarations = fs.readFileSync(new URL("../index.d.ts", import.meta.url), "utf8");
  for (const retired of [
    "SccpDetachedSigningState",
    "SccpBridgeProofSubmitPayload",
    "SccpBridgeMessageSubmitPayload",
    "SccpBridgeSubmitResponse",
    "submitBridgeProof",
    "submitBridgeMessage",
  ]) {
    assert.doesNotMatch(declarations, new RegExp(`\\b${retired}\\b`, "u"));
  }
});
