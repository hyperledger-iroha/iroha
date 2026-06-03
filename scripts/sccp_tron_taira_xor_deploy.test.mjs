#!/usr/bin/env node
// Unit tests for the TAIRA XOR TRON deployment helper's offline validation
// paths. These tests do not contact TRON and must never broadcast.
import assert from "node:assert/strict";
import { chmod, mkdtemp, readFile, rm, stat, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import { secp256k1 } from "../javascript/iroha_js/node_modules/@noble/curves/secp256k1.js";
import { sha256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha256.js";
import {
  ASSET_KEY,
  ROUTE_ID,
  TAIRA_BURN_RECORD_ARTIFACT_MAX_BYTES,
  TRON_MAINNET_NETWORK_ID_HEX,
  TRON_NILE_NETWORK_ID_HEX,
  assertDeploymentFundingReady,
  buildDeploymentDoctorReport,
  buildDeploymentConfigurationSpecs,
  buildDeploymentFundingReadiness,
  buildSignedTransactionArtifact,
  buildUnsignedTransactionArtifact,
  buildTairaXorRouteManifestDraft,
  bytesToHex,
  compileTairaBurnRecordContract,
  estimateDeploymentFunding,
  generateDeployer,
  hexToBytes,
  normalizeTronAddress,
  normalizeTronBase58Address,
  normalizeTronEndpoint,
  normalizeTronNetwork,
  normalizeSignedTransactionArtifact,
  normalizeUnsignedTransactionArtifact,
  normalizeVerifierConstructorArgs,
  routeHash,
  signTransactionPayload,
  tronDestinationBindingHash,
  tronDestinationBindingKey,
  tronAddressFromPrivateKey,
  verifySignedTransactionPayload,
} from "./sccp_tron_taira_xor_deploy.mjs";

const privateKey = new Uint8Array(32).fill(7);
const deployerAddress = tronAddressFromPrivateKey(privateKey);
const routeAddresses = {
  token: tronAddressFromPrivateKey(new Uint8Array(32).fill(1)),
  bridge: tronAddressFromPrivateKey(new Uint8Array(32).fill(2)),
  sourceBridge: tronAddressFromPrivateKey(new Uint8Array(32).fill(3)),
  verifier: tronAddressFromPrivateKey(new Uint8Array(32).fill(4)),
};
const burnArtifactBytes = Buffer.from("Nrt0fixture-bytecode-material-v1!!");
const burnArtifactB64 = Buffer.from(burnArtifactBytes).toString("base64");
const burnArtifactSha256 = bytesToHex(sha256(burnArtifactBytes));
const validMnemonic =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const pemPrivateKey =
  "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n-----END PRIVATE KEY-----";
const requiredPostDeployChecks = [
  "TairaXOR.bridge() equals taira_xor_bridge_address",
  "TairaXOR.bridgeLocked() is true",
  "SccpTronSourceBridge.owner() equals taira_xor_bridge_address",
  "TairaXorSccpBridge.destinationBindingHash() equals verifier destinationBindingHash()",
  "Run scripts/sccp_tron_source_bridge_evidence.py for source bridge config evidence",
  "Run scripts/sccp_tron_live_evidence.py for live verifier/source/canary evidence",
];

const mockTransaction = (ownerAddress = deployerAddress.base58, rawData = [1, 2, 3, 4]) => {
  const rawBytes = Uint8Array.from(rawData);
  return {
    visible: true,
    txID: bytesToHex(sha256(rawBytes), false),
    raw_data: {
      contract: [
        {
          parameter: {
            value: {
              owner_address: ownerAddress,
            },
          },
          type: "TriggerSmartContract",
        },
      ],
      timestamp: 1,
      expiration: 2,
    },
    raw_data_hex: bytesToHex(rawBytes, false),
  };
};

const verifierMaterial = () => ({
  alpha1: [1, 2],
  beta2: [
    [3, 4],
    [5, 6],
  ],
  gamma2: [7, 8, 9, 10],
  delta2: [11, 12, 13, 14],
  ic: [15, 16, 17, 18],
  verifierKeyHash: routeHash("verifier-key"),
});

const withTempDir = async (fn) => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-sccp-tron-deploy-"));
  try {
    return await fn(dir);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
};

const writeJson = async (path, value, mode) => {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, {
    ...(mode ? { mode } : {}),
  });
  if (mode) {
    await chmod(path, mode);
  }
};

const deployerSecretRecord = () => ({
  schema: "iroha-sccp-tron-taira-xor-deployer/v1",
  created_at: "2026-06-01T00:00:00.000Z",
  network: "tron-mainnet",
  address_base58: deployerAddress.base58,
  address_hex: deployerAddress.hex,
  private_key_hex: bytesToHex(privateKey, false),
});

const writeDeployerSecret = async (path) => {
  await writeJson(path, deployerSecretRecord(), 0o600);
};

const routeDeploymentEvidence = (overrides = {}) => ({
  schema: "iroha-sccp-tron-taira-xor-deployment-evidence/v1",
  created_at: "2026-06-01T00:00:00.000Z",
  route_id: "taira_tron_xor",
  route_id_hash: routeHash("taira_tron_xor"),
  asset_key: "xor",
  asset_key_hash: routeHash("xor"),
  network: "tron-mainnet",
  network_id_hex: TRON_MAINNET_NETWORK_ID_HEX,
  taira_xor_token_address: routeAddresses.token.base58,
  taira_xor_token_address_hex: routeAddresses.token.hex,
  taira_xor_bridge_address: routeAddresses.bridge.base58,
  taira_xor_bridge_address_hex: routeAddresses.bridge.hex,
  sccp_tron_source_bridge_address: routeAddresses.sourceBridge.base58,
  sccp_tron_source_bridge_address_hex: routeAddresses.sourceBridge.hex,
  sccp_tron_destination_verifier_address: routeAddresses.verifier.base58,
  sccp_tron_destination_verifier_address_hex: routeAddresses.verifier.hex,
  required_post_deploy_checks: [...requiredPostDeployChecks],
  ...overrides,
});

const tairaBurnRecordContract = (overrides = {}) => ({
  schema: "iroha-sccp-taira-xor-burn-record-contract/v1",
  created_at: "2026-06-01T00:00:00.000Z",
  route_id: "taira_tron_xor",
  asset_key: "xor",
  source_name: "contracts/taira/sccp/TairaXorSccpBurnRecord.ko",
  compiler_fingerprint: "kotodama_lang/test",
  code_hash: routeHash("taira-burn-record-code").slice(2),
  abi_hash: routeHash("taira-burn-record-abi").slice(2),
  artifact_sha256: burnArtifactSha256,
  artifact_b64: burnArtifactB64,
  manifest: {
    features_bitmap: 1,
  },
  execution: {
    executable: "IvmProved",
    force_zk_mode: true,
    entrypoint: "burn_and_record",
  },
  ...overrides,
});

const routeLiveEvidence = ({
  verifierCodeHash = routeHash("deployed-verifier-code"),
  verifierKeyHash = routeHash("verifier-key"),
  destinationBindingHash,
  destinationBindingKey,
  sourceBridge = {},
  destinationVerifier = {},
  routeCanary = {},
  routeCanaryTransaction = {},
  sourceEventTransaction = {},
  triggerContract = {},
  summary = {},
} = {}) => {
  const bindingHash =
    destinationBindingHash ??
    tronDestinationBindingHash({
      networkId: TRON_MAINNET_NETWORK_ID_HEX,
      verifierAddress: routeAddresses.verifier.base58,
      verifierCodeHash,
      verifierKeyHash,
    });
  const bindingKey =
    destinationBindingKey ??
    tronDestinationBindingKey({
      networkId: TRON_MAINNET_NETWORK_ID_HEX,
      verifierAddress: routeAddresses.verifier.base58,
      verifierCodeHash,
      verifierKeyHash,
    });
  const canaryHash = routeHash("route-canary-evidence");
  const canaryMessageId = routeHash("route-canary-message");
  const canaryCommitmentRoot = routeHash("route-canary-commitment-root");
  const canaryStatementHash = routeHash("route-canary-statement-hash");
  return {
    full_toml_ready: true,
    source_bridge: {
      address: routeAddresses.sourceBridge.base58,
      source_bridge_network_id: TRON_MAINNET_NETWORK_ID_HEX,
      source_domain: 5,
      target_domain: 0,
      source_bridge_owner_base58: routeAddresses.bridge.base58,
      source_bridge_config_hash: routeHash("source-bridge-config"),
      config_hash_matches: true,
      ...sourceBridge,
    },
    destination_verifier: {
      address: routeAddresses.verifier.base58,
      network_id: TRON_MAINNET_NETWORK_ID_HEX,
      destination_source_domain: 0,
      destination_target_domain: 5,
      destination_verifier_code_hash: verifierCodeHash,
      destination_verifier_key_hash: verifierKeyHash,
      verifier_backend_hash_matches: true,
      proof_family_hash_matches: true,
      destination_binding_hash: bindingHash,
      recomputed_destination_binding_hash: bindingHash,
      destination_binding_key: bindingKey,
      destination_binding_hash_matches: true,
      expected_destination_binding_hash_matches: true,
      bytecode_hash_matches_verifier_code_hash: true,
      ...destinationVerifier,
    },
    route_canary: {
      status: "passed",
      evidence_source: "tron_message_proof_accepted_transaction",
      evidence_hash: canaryHash,
      ...routeCanary,
    },
    route_canary_transaction: {
      transaction_id: routeHash("route-canary-transaction"),
      source_domain: 0,
      message_id: canaryMessageId,
      commitment_root: canaryCommitmentRoot,
      statement_hash: canaryStatementHash,
      destination_binding_hash: bindingHash,
      network_id: TRON_MAINNET_NETWORK_ID_HEX,
      message_proof_used: true,
      route_canary_evidence_hash: canaryHash,
      trigger_contract: {
        raw_data_owner_matches_transaction: true,
        signature_recovers_to_owner: true,
        raw_data_call_matches: true,
        proof_source_domain: 0,
        public_inputs_target_domain: 5,
        public_inputs_message_id: canaryMessageId,
        public_inputs_commitment_root: canaryCommitmentRoot,
        statement_hash: canaryStatementHash,
        call_matches: true,
        contract_address: routeAddresses.verifier.hex,
        contract_base58: routeAddresses.verifier.base58,
        ...triggerContract,
      },
      ...routeCanaryTransaction,
    },
    source_event_transaction: {
      transaction_id: routeHash("source-event-transaction"),
      source_event_transaction_production_ready: true,
      ...sourceEventTransaction,
    },
    offline_full_toml_sha256: routeHash("offline-full-toml"),
    ...summary,
  };
};

const writeRouteManifestInputs = async (dir, { evidence = {}, contract = {}, verifier = verifierMaterial() } = {}) => {
  const evidencePath = join(dir, "deployment.evidence.json");
  const contractPath = join(dir, "burn-record.contract.json");
  const verifierPath = join(dir, "verifier.json");
  await writeJson(evidencePath, routeDeploymentEvidence(evidence));
  await writeJson(contractPath, tairaBurnRecordContract(contract));
  await writeJson(verifierPath, verifier);
  return { evidencePath, contractPath, verifierPath };
};

test("normalizes canonical TRON Base58, 21-byte hex, and Solidity address forms", () => {
  const fromBase58 = normalizeTronBase58Address(deployerAddress.base58, "base58");
  assert.equal(fromBase58.hex, deployerAddress.hex);
  assert.equal(fromBase58.solidity, `0x${deployerAddress.hex.slice(4)}`);

  const fromHex = normalizeTronAddress(deployerAddress.hex, "hex");
  assert.equal(fromHex.base58, deployerAddress.base58);
  assert.equal(fromHex.solidity, fromBase58.solidity);

  const fromSolidity = normalizeTronAddress(fromBase58.solidity, "solidity");
  assert.equal(fromSolidity.base58, deployerAddress.base58);
});

test("rejects malformed, non-canonical, checksum-invalid, and zero TRON addresses", () => {
  assert.throws(() => normalizeTronBase58Address(` ${deployerAddress.base58}`, "address"), /canonical/);
  assert.throws(
    () =>
      normalizeTronBase58Address(
        `${deployerAddress.base58.slice(0, -1)}${deployerAddress.base58.endsWith("1") ? "2" : "1"}`,
        "address",
      ),
    /checksum/,
  );
  assert.throws(() => normalizeTronAddress("0x410000000000000000000000000000000000000000", "address"), /non-zero/);
  assert.throws(() => normalizeTronAddress("not-an-address", "address"), /hex|Base58/);
});

test("offline signing binds txID, raw_data owner, and recovered signer", () => {
  const transaction = mockTransaction();
  const signed = signTransactionPayload(transaction, { privateKey, address: deployerAddress });
  assert.equal(signed.metadata.txid, transaction.txID);
  assert.equal(signed.metadata.signature_recovers_to_owner, true);
  assert.equal(signed.metadata.signature_recovered_base58, deployerAddress.base58);
  assert.match(signed.signed.signature[0], /^[0-9a-f]{130}$/u);
});

test("unsigned transaction artifacts are route scoped before deployer signing", () => {
  const deployer = { privateKey, address: deployerAddress };
  const transaction = mockTransaction();
  const artifact = buildUnsignedTransactionArtifact(
    {
      stepKey: "token_set_bridge",
      stepKind: "trigger",
      deployerAddress,
      transaction,
    },
    new Date("2026-06-01T00:00:00.000Z"),
  );

  assert.equal(artifact.schema, "iroha-sccp-tron-unsigned-transaction/v1");
  assert.equal(artifact.network, "tron-mainnet");
  assert.equal(artifact.network_id_hex, TRON_MAINNET_NETWORK_ID_HEX);
  assert.equal(artifact.route_id, ROUTE_ID);
  assert.equal(artifact.asset_key, ASSET_KEY);
  assert.equal(artifact.purpose, "taira-xor-sccp-deployment");
  assert.equal(artifact.step_key, "token_set_bridge");
  assert.equal(artifact.step_kind, "trigger");
  assert.equal(artifact.txid, transaction.txID);

  const normalized = normalizeUnsignedTransactionArtifact(
    artifact,
    {},
    deployer,
  );
  assert.equal(normalized.transaction.txID, transaction.txID);
  assert.equal(normalized.stepKey, "token_set_bridge");
  assert.equal(normalized.stepKind, "trigger");

  assert.throws(
    () => normalizeUnsignedTransactionArtifact(transaction, {}, deployer),
    /route-scoped unsigned artifact|dry-run deployment plan/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...artifact, schema: "tron-unsigned-transaction/v1" },
        {},
        deployer,
      ),
    /route-scoped unsigned artifact|dry-run deployment plan/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...artifact, network: "tron-testnet" },
        {},
        deployer,
      ),
    /network/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...artifact, route_id: "other_route" },
        {},
        deployer,
      ),
    /route_id/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...artifact, asset_key: "sora" },
        {},
        deployer,
      ),
    /asset_key/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...artifact, purpose: "end-user-bridge" },
        {},
        deployer,
      ),
    /purpose/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...artifact, step_kind: "transfer" },
        {},
        deployer,
      ),
    /step_kind/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...artifact, txid: "00".repeat(32) },
        {},
        deployer,
      ),
    /txid/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...artifact, deployer_address_base58: routeAddresses.token.base58 },
        {},
        deployer,
      ),
    /deployer address/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...artifact, operator_note: validMnemonic },
        {},
        deployer,
      ),
    /operator_note.*recovery phrases/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        {
          ...artifact,
          transaction: {
            ...artifact.transaction,
            raw_data: {
              ...artifact.transaction.raw_data,
              memo: pemPrivateKey,
            },
          },
        },
        {},
        deployer,
      ),
    /memo.*private key material/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...artifact, debug_private_key: bytesToHex(privateKey, false) },
        {},
        deployer,
      ),
    /debug_private_key.*deployment artifacts/u,
  );
});

test("dry-run deployment plans require an explicit unique route-scoped signing step", () => {
  const deployer = { privateKey, address: deployerAddress };
  const transaction = mockTransaction();
  const unsignedArtifact = buildUnsignedTransactionArtifact(
    {
      stepKey: "token_set_bridge",
      stepKind: "trigger",
      deployerAddress,
      transaction,
    },
    new Date("2026-06-01T00:00:00.000Z"),
  );
  const plan = {
    schema: "iroha-sccp-tron-taira-xor-deployment-plan/v1",
    network: "tron-mainnet",
    network_id_hex: TRON_MAINNET_NETWORK_ID_HEX,
    route_id: ROUTE_ID,
    asset_key: ASSET_KEY,
    broadcast: false,
    deployer_address_base58: deployerAddress.base58,
    deployer_address_hex: deployerAddress.hex,
    steps: [
      {
        kind: "trigger",
        key: "token_set_bridge",
        unsigned_transaction: transaction,
        unsigned_artifact: unsignedArtifact,
      },
    ],
  };

  const normalized = normalizeUnsignedTransactionArtifact(
    plan,
    { step: "token_set_bridge" },
    deployer,
  );
  assert.equal(normalized.transaction.txID, transaction.txID);
  assert.equal(normalized.stepKey, "token_set_bridge");
  assert.equal(normalized.stepKind, "trigger");

  assert.throws(
    () => normalizeUnsignedTransactionArtifact(plan, {}, deployer),
    /--step/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...plan, broadcast: true },
        { step: "token_set_bridge" },
        deployer,
      ),
    /broadcast false/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...plan, route_id: "other_route" },
        { step: "token_set_bridge" },
        deployer,
      ),
    /route_id/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...plan, deployer_address_hex: routeAddresses.token.hex },
        { step: "token_set_bridge" },
        deployer,
      ),
    /deployer address/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...plan, steps: [] },
        { step: "token_set_bridge" },
        deployer,
      ),
    /exactly one step/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        { ...plan, steps: [...plan.steps, ...plan.steps] },
        { step: "token_set_bridge" },
        deployer,
      ),
    /exactly one step/u,
  );
  assert.throws(
    () =>
      normalizeUnsignedTransactionArtifact(
        {
          ...plan,
          steps: [
            {
              ...plan.steps[0],
              unsigned_artifact: { ...unsignedArtifact, step_key: "wrong_step" },
            },
          ],
        },
        { step: "token_set_bridge" },
        deployer,
      ),
    /step metadata/u,
  );
});

test("signed transaction verification recovers the owner before broadcast", () => {
  const transaction = mockTransaction();
  const signed = signTransactionPayload(transaction, { privateKey, address: deployerAddress });
  const verified = verifySignedTransactionPayload(signed.signed);

  assert.equal(verified.txid, transaction.txID);
  assert.equal(verified.owner_base58, deployerAddress.base58);
  assert.equal(verified.signature_recovered_base58, deployerAddress.base58);
  assert.equal(verified.signature_recovers_to_owner, true);

  const recovery27Transaction = JSON.parse(JSON.stringify(signed.signed));
  const recovery27Signature = hexToBytes(recovery27Transaction.signature[0], "signature", 65);
  recovery27Signature[64] += 27;
  recovery27Transaction.signature = [bytesToHex(recovery27Signature, false)];
  const recovery27Verified = verifySignedTransactionPayload(recovery27Transaction);
  assert.equal(recovery27Verified.owner_base58, deployerAddress.base58);
  assert.equal(recovery27Verified.signature_recovered_base58, deployerAddress.base58);
});

test("signed transaction artifacts are route scoped and metadata bound before broadcast", () => {
  const transaction = mockTransaction();
  const signed = signTransactionPayload(transaction, { privateKey, address: deployerAddress });
  const artifact = buildSignedTransactionArtifact(signed, new Date("2026-06-01T00:00:00.000Z"));

  assert.equal(artifact.schema, "iroha-sccp-tron-signed-transaction/v1");
  assert.equal(artifact.network, "tron-mainnet");
  assert.equal(artifact.network_id_hex, TRON_MAINNET_NETWORK_ID_HEX);
  assert.equal(artifact.route_id, ROUTE_ID);
  assert.equal(artifact.asset_key, ASSET_KEY);
  assert.equal(artifact.purpose, "taira-xor-sccp-deployment");

  const normalized = normalizeSignedTransactionArtifact(artifact);
  assert.equal(normalized.transaction.signature[0], signed.metadata.signature);
  assert.equal(normalized.verified.txid, signed.metadata.txid);
  assert.equal(normalized.verified.owner_base58, deployerAddress.base58);

  assert.throws(() => normalizeSignedTransactionArtifact(signed.signed), /schema/u);
  assert.throws(
    () => normalizeSignedTransactionArtifact({ ...artifact, schema: "tron-signed-transaction/v1" }),
    /schema/u,
  );
  assert.throws(
    () => normalizeSignedTransactionArtifact({ ...artifact, network: "tron-testnet" }),
    /network/u,
  );
  assert.throws(
    () => normalizeSignedTransactionArtifact({ ...artifact, network_id_hex: routeHash("wrong-network") }),
    /network_id_hex/u,
  );
  assert.throws(
    () => normalizeSignedTransactionArtifact({ ...artifact, route_id: "taira_tron_sora" }),
    /route_id/u,
  );
  assert.throws(
    () => normalizeSignedTransactionArtifact({ ...artifact, asset_key: "sora" }),
    /asset_key/u,
  );
  assert.throws(
    () => normalizeSignedTransactionArtifact({ ...artifact, purpose: "end-user-bridge" }),
    /purpose/u,
  );
  assert.throws(
    () => normalizeSignedTransactionArtifact({ ...artifact, txid: "00".repeat(32) }),
    /txid/u,
  );
  assert.throws(
    () => normalizeSignedTransactionArtifact({ ...artifact, signature: "00".repeat(65) }),
    /signature/u,
  );
  assert.throws(
    () => normalizeSignedTransactionArtifact({ ...artifact, owner_base58: routeAddresses.token.base58 }),
    /owner_base58/u,
  );
  assert.throws(
    () => normalizeSignedTransactionArtifact({ ...artifact, signature_recovers_to_owner: false }),
    /signature_recovers_to_owner/u,
  );
  assert.throws(
    () => normalizeSignedTransactionArtifact({ ...artifact, operator_note: validMnemonic }),
    /operator_note.*recovery phrases/u,
  );
  assert.throws(
    () =>
      normalizeSignedTransactionArtifact({
        ...artifact,
        transaction: {
          ...artifact.transaction,
          raw_data: {
            ...artifact.transaction.raw_data,
            memo: pemPrivateKey,
          },
        },
      }),
    /memo.*private key material/u,
  );
  assert.throws(
    () =>
      normalizeSignedTransactionArtifact({
        ...artifact,
        deployer_secret: bytesToHex(privateKey, false),
      }),
    /deployer_secret.*deployment artifacts/u,
  );
});

test("signed transaction verification rejects forged or ambiguous signatures", () => {
  const transaction = mockTransaction();
  const signed = signTransactionPayload(transaction, { privateKey, address: deployerAddress });

  const otherPrivateKey = new Uint8Array(32).fill(8);
  const otherSignature = secp256k1.sign(
    sha256(hexToBytes(transaction.raw_data_hex, "raw_data_hex")),
    otherPrivateKey,
    { prehash: false, lowS: true },
  );
  const otherCompact = otherSignature.toCompactRawBytes();
  const wrongSignerBytes = new Uint8Array(65);
  wrongSignerBytes.set(otherCompact);
  wrongSignerBytes[64] = otherSignature.recovery;
  assert.throws(
    () =>
      verifySignedTransactionPayload({
        ...transaction,
        signature: [bytesToHex(wrongSignerBytes, false)],
      }),
    /not owner/u,
  );

  assert.throws(
    () =>
      verifySignedTransactionPayload({
        ...transaction,
        signature: ["00".repeat(64)],
      }),
    /65 bytes/u,
  );

  assert.throws(
    () =>
      verifySignedTransactionPayload({
        ...transaction,
        signature: [signed.metadata.signature, signed.metadata.signature],
      }),
    /exactly one signature/u,
  );

  const nonCanonical = JSON.parse(JSON.stringify(signed.signed));
  const nonCanonicalBytes = hexToBytes(nonCanonical.signature[0], "signature", 65);
  nonCanonicalBytes[64] = 31;
  nonCanonical.signature = [bytesToHex(nonCanonicalBytes, false)];
  assert.throws(
    () => verifySignedTransactionPayload(nonCanonical),
    /canonical recoverable/u,
  );
});

test("offline signing rejects txid mismatch, wrong owner, duplicate signatures, and malformed owners", () => {
  assert.throws(
    () =>
      signTransactionPayload(
        {
          ...mockTransaction(),
          txID: "00".repeat(32),
        },
        { privateKey, address: deployerAddress },
      ),
    /txID/,
  );

  const otherAddress = tronAddressFromPrivateKey(new Uint8Array(32).fill(8));
  assert.throws(
    () => signTransactionPayload(mockTransaction(otherAddress.base58), { privateKey, address: deployerAddress }),
    /does not match deployer/,
  );

  assert.throws(
    () =>
      signTransactionPayload(
        {
          ...mockTransaction(),
          signature: ["00".repeat(65)],
        },
        { privateKey, address: deployerAddress },
      ),
    /already signed/,
  );

  assert.throws(
    () => signTransactionPayload(mockTransaction("bad-owner"), { privateKey, address: deployerAddress }),
    /hex|Base58/,
  );
});

test("verifier material normalization accepts production lane material", () => {
  const args = normalizeVerifierConstructorArgs(verifierMaterial());
  assert.equal(args.length, 10);
  assert.deepEqual(args[0], ["1", "2"]);
  assert.equal(args[5], routeHash("verifier-key"));
  assert.equal(args[6], "stark-fri-v1");
  assert.equal(args[7], TRON_MAINNET_NETWORK_ID_HEX);
  assert.equal(args[8], 0);
  assert.equal(args[9], 5);
});

test("verifier material normalization rejects stale or adversarial lane material", () => {
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), proofFamily: "debug-proof-family" }),
    /proofFamily/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), networkId: routeHash("wrong-network") }),
    /networkId/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), sourceDomain: 5, targetDomain: 0 }),
    /SORA -> TRON/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), beta2: [1, 2, 3] }),
    /beta2/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), verifierKeyHash: "0x00" }),
    /32 bytes/,
  );
});

test("hex helper rejects odd, non-hex, and wrong-length values", () => {
  assert.deepEqual([...hexToBytes("0x0102", "sample", 2)], [1, 2]);
  assert.throws(() => hexToBytes("0x1", "sample"), /even-length/);
  assert.throws(() => hexToBytes("0xzz", "sample"), /hex/);
  assert.throws(() => hexToBytes("0x0102", "sample", 3), /3 bytes/);
});

test("generated deployer private key still derives a valid TRON address shape", () => {
  const generated = tronAddressFromPrivateKey(secp256k1.utils.randomPrivateKey());
  assert.match(generated.base58, /^T[1-9A-HJ-NP-Za-km-z]{33}$/u);
  assert.match(generated.hex, /^0x41[0-9a-f]{40}$/u);
});

test("generate-deployer writes restrictive secret files and refuses accidental overwrite", async () => {
  await withTempDir(async (dir) => {
    const secretPath = join(dir, "deployer.secret.json");
    const originalConsoleLog = console.log;
    console.log = () => {};
    try {
      await generateDeployer({ out: secretPath });
      const firstSecretText = await readFile(secretPath, "utf8");
      const firstSecret = JSON.parse(firstSecretText);
      const mode = (await stat(secretPath)).mode & 0o777;

      assert.equal(firstSecret.schema, "iroha-sccp-tron-taira-xor-deployer/v1");
      assert.match(firstSecret.address_base58, /^T[1-9A-HJ-NP-Za-km-z]{33}$/u);
      assert.equal(mode, 0o600);
      assert.equal(firstSecretText.includes("private_key_hex"), true);

      await assert.rejects(
        () => generateDeployer({ out: secretPath }),
        /Refusing to overwrite existing deployer secret/u,
      );
      assert.equal(await readFile(secretPath, "utf8"), firstSecretText);

      await generateDeployer({ out: secretPath, force: "true" });
      const rotatedSecretText = await readFile(secretPath, "utf8");
      const rotatedSecret = JSON.parse(rotatedSecretText);
      assert.notEqual(rotatedSecret.private_key_hex, firstSecret.private_key_hex);
      assert.equal((await stat(secretPath)).mode & 0o777, 0o600);

      const nileSecretPath = join(dir, "nile-deployer.secret.json");
      await generateDeployer({ out: nileSecretPath, "tron-network": "nile" });
      const nileSecret = JSON.parse(await readFile(nileSecretPath, "utf8"));
      assert.equal(nileSecret.tron_network, "nile");
      assert.equal(nileSecret.network, "tron-nile");
      assert.equal(nileSecret.chain_id_hex, "0xcd8690dc");
      assert.equal(nileSecret.network_id_hex, TRON_NILE_NETWORK_ID_HEX);
      assert.equal(nileSecret.endpoint, "https://nile.trongrid.io");
      assert.match(nileSecret.address_base58, /^T[1-9A-HJ-NP-Za-km-z]{33}$/u);
      assert.equal((await stat(nileSecretPath)).mode & 0o777, 0o600);
    } finally {
      console.log = originalConsoleLog;
    }
  });
});

test("deployment configuration specs define the required post-deploy trigger order", () => {
  const specs = buildDeploymentConfigurationSpecs({
    tokenAddress: routeAddresses.token,
    sourceBridgeAddress: routeAddresses.sourceBridge,
    verifierAddress: routeAddresses.verifier,
    bridgeAddress: routeAddresses.bridge,
  });

  assert.deepEqual(
    specs.map((spec) => spec.key),
    [
      "token_set_bridge",
      "token_lock_bridge",
      "source_bridge_transfer_ownership",
      "verifier_emit_destination_binding",
    ],
  );
  assert.deepEqual(
    specs.map((spec) => spec.contractKey),
    ["token", "token", "source_bridge", "verifier"],
  );
  assert.deepEqual(
    specs.map((spec) => spec.functionName),
    ["setBridge", "lockBridge", "transferOwnership", "emitDestinationBindingConfigured"],
  );
  assert.deepEqual(specs.map((spec) => spec.args), [
    [routeAddresses.bridge.solidity],
    [],
    [routeAddresses.bridge.solidity],
    [],
  ]);
  assert.deepEqual(
    specs.map((spec) => spec.requiredPostDeployCheck),
    [
      "TairaXOR.bridge() equals taira_xor_bridge_address",
      "TairaXOR.bridgeLocked() is true",
      "SccpTronSourceBridge.owner() equals taira_xor_bridge_address",
      "TairaXorSccpBridge.destinationBindingHash() equals verifier destinationBindingHash()",
    ],
  );
  assert.deepEqual(specs[0].contractAddress, {
    base58: routeAddresses.token.base58,
    hex: routeAddresses.token.hex,
    solidity: routeAddresses.token.solidity,
  });
  assert.deepEqual(specs[2].contractAddress, {
    base58: routeAddresses.sourceBridge.base58,
    hex: routeAddresses.sourceBridge.hex,
    solidity: routeAddresses.sourceBridge.solidity,
  });
  assert.equal(JSON.stringify(specs).includes("private_key"), false);
  assert.equal(JSON.stringify(specs).includes("payload"), false);
});

test("deployment funding estimate is a conservative mainnet TRX and energy budget", () => {
  const estimate = estimateDeploymentFunding();
  assert.equal(
    estimate.post_deploy_trigger_transaction_count,
    buildDeploymentConfigurationSpecs({
      tokenAddress: routeAddresses.token.base58,
      sourceBridgeAddress: routeAddresses.sourceBridge.hex,
      verifierAddress: routeAddresses.verifier.base58,
      bridgeAddress: routeAddresses.bridge.solidity,
    }).length,
  );
  assert.equal(estimate.schema, "iroha-sccp-tron-taira-xor-funding-estimate/v1");
  assert.equal(estimate.route_id, "taira_tron_xor");
  assert.equal(estimate.network, "tron-mainnet");
  assert.equal(estimate.funding_mode, "aggregate");
  assert.equal(estimate.deployment_transaction_count, 4);
  assert.equal(estimate.post_deploy_trigger_transaction_count, 4);
  assert.equal(estimate.deploy_fee_limit_sun, "500000000");
  assert.equal(estimate.trigger_fee_limit_sun, "20000000");
  assert.equal(estimate.max_single_fee_limit_sun, "500000000");
  assert.equal(estimate.max_single_fee_limit_trx, "500");
  assert.equal(estimate.total_fee_limit_sun, "2080000000");
  assert.equal(estimate.total_fee_limit_trx, "2080");
  assert.equal(estimate.safety_margin_percent, 15);
  assert.equal(estimate.aggregate_recommended_min_balance_sun, "2392000000");
  assert.equal(estimate.aggregate_recommended_min_balance_trx, "2392");
  assert.equal(estimate.staged_safety_margin_sun, "75000000");
  assert.equal(estimate.staged_recommended_min_balance_sun, "575000000");
  assert.equal(estimate.staged_recommended_min_balance_trx, "575");
  assert.equal(estimate.recommended_min_balance_sun, "2392000000");
  assert.equal(estimate.recommended_min_balance_trx, "2392");
  assert.equal(estimate.origin_energy_limit_per_deploy, "10000000");
  assert.equal(estimate.max_origin_energy_limit_total, "40000000");
});

test("deployment funding estimate supports explicit operator fee limits", () => {
  const estimate = estimateDeploymentFunding({
    "fee-limit": "2000000",
    "trigger-fee-limit": "500000",
    "origin-energy-limit": "12345",
    "safety-margin-percent": "10",
  });
  assert.equal(estimate.funding_mode, "aggregate");
  assert.equal(estimate.max_single_fee_limit_sun, "2000000");
  assert.equal(estimate.max_single_fee_limit_trx, "2");
  assert.equal(estimate.total_fee_limit_sun, "10000000");
  assert.equal(estimate.total_fee_limit_trx, "10");
  assert.equal(estimate.safety_margin_sun, "1000000");
  assert.equal(estimate.aggregate_recommended_min_balance_sun, "11000000");
  assert.equal(estimate.staged_recommended_min_balance_sun, "2200000");
  assert.equal(estimate.recommended_min_balance_sun, "11000000");
  assert.equal(estimate.recommended_min_balance_trx, "11");
  assert.equal(estimate.max_origin_energy_limit_total, "49380");
});

test("deployment funding estimate supports staged operator funding", () => {
  const estimate = estimateDeploymentFunding({
    "fee-limit": "2000000",
    "trigger-fee-limit": "500000",
    "safety-margin-percent": "10",
    "funding-mode": "staged",
  });
  assert.equal(estimate.funding_mode, "staged");
  assert.equal(estimate.aggregate_recommended_min_balance_sun, "11000000");
  assert.equal(estimate.staged_safety_margin_sun, "200000");
  assert.equal(estimate.staged_recommended_min_balance_sun, "2200000");
  assert.equal(estimate.staged_recommended_min_balance_trx, "2.2");
  assert.equal(estimate.recommended_min_balance_sun, "2200000");
  assert.equal(estimate.recommended_min_balance_trx, "2.2");
  assert.match(estimate.assumptions.join("\n"), /Staged mode/u);
});

test("TRON testnet profiles bind funding and verifier material to Nile", () => {
  assert.equal(normalizeTronNetwork("nile"), "nile");
  assert.equal(normalizeTronNetwork("tron-nile"), "nile");
  assert.throws(() => normalizeTronNetwork("polygon"), /tron-network/u);

  const estimate = estimateDeploymentFunding({
    "tron-network": "nile",
    "funding-mode": "staged",
  });
  assert.equal(estimate.tron_network, "nile");
  assert.equal(estimate.network, "tron-nile");
  assert.equal(estimate.chain_id_hex, "0xcd8690dc");
  assert.equal(estimate.network_id_hex, TRON_NILE_NETWORK_ID_HEX);
  assert.equal(estimate.recommended_min_balance_trx, "575");

  const nileVerifierArgs = normalizeVerifierConstructorArgs(verifierMaterial(), {
    "tron-network": "nile",
  });
  assert.equal(nileVerifierArgs[7], TRON_NILE_NETWORK_ID_HEX);
  assert.throws(
    () =>
      normalizeVerifierConstructorArgs(
        { ...verifierMaterial(), networkId: TRON_MAINNET_NETWORK_ID_HEX },
        { "tron-network": "nile" },
      ),
    /tron-nile/u,
  );
});

test("deployment funding estimate rejects unsafe fee and margin inputs", () => {
  assert.throws(() => estimateDeploymentFunding({ "fee-limit": "0" }), /fee-limit/);
  assert.throws(
    () => estimateDeploymentFunding({ "trigger-fee-limit": "1.5" }),
    /trigger-fee-limit/,
  );
  assert.throws(
    () => estimateDeploymentFunding({ "origin-energy-limit": "-1" }),
    /origin-energy-limit/,
  );
  assert.throws(
    () => estimateDeploymentFunding({ "safety-margin-percent": "101" }),
    /safety-margin-percent/,
  );
  assert.throws(
    () => estimateDeploymentFunding({ "funding-mode": "single-step" }),
    /funding-mode/,
  );
});

test("TRON endpoint normalization rejects unsafe deployment gateway overrides", () => {
  assert.equal(normalizeTronEndpoint("https://api.trongrid.io/"), "https://api.trongrid.io");
  assert.equal(
    normalizeTronEndpoint("https://api.trongrid.io/custom/path/"),
    "https://api.trongrid.io/custom/path",
  );
  assert.throws(() => normalizeTronEndpoint("http://api.trongrid.io"), /HTTPS/);
  assert.throws(() => normalizeTronEndpoint("https://user:pass@api.trongrid.io"), /credentials/);
  assert.throws(() => normalizeTronEndpoint("https://api.trongrid.io?api_key=secret"), /query/);
  assert.throws(() => normalizeTronEndpoint("https://api.trongrid.io/#fragment"), /fragment|query/);
  assert.throws(() => normalizeTronEndpoint("https://localhost"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://node.localhost"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://local"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://0.0.0.0"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://127.0.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://10.0.0.7"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://100.64.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://172.16.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://192.0.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://192.168.1.2"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://198.18.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://224.0.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[::1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[fd00::1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[fe80::1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[::ffff:127.0.0.1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[::7f00:1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[64:ff9b::7f00:1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[2002:7f00:0001::1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[2001:0000:7f00:0001::1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[2001:db8::1]"), /localhost|private/);
});

test("deployment funding readiness reports gaps and fails underfunded broadcast preflight", () => {
  const options = {
    "fee-limit": "2000000",
    "trigger-fee-limit": "500000",
    "origin-energy-limit": "12345",
    "safety-margin-percent": "10",
  };
  const ready = buildDeploymentFundingReadiness({ balance: "11000000" }, options);
  assert.equal(ready.schema, "iroha-sccp-tron-taira-xor-funding-readiness/v1");
  assert.equal(ready.funding_ready, true);
  assert.equal(ready.funding_gap_sun, "0");
  assert.equal(assertDeploymentFundingReady({ balance: "11000000" }, options).funding_ready, true);

  const underfunded = buildDeploymentFundingReadiness({ balance: "10999999" }, options);
  assert.equal(underfunded.funding_ready, false);
  assert.equal(underfunded.funding_gap_sun, "1");
  assert.throws(
    () => assertDeploymentFundingReady({ balance: "10999999" }, options),
    /below the recommended minimum.*gap=1 SUN/u,
  );
  assert.throws(
    () => buildDeploymentFundingReadiness({ balance: "-1" }, options),
    /account\.balance/,
  );

  const stagedOptions = { ...options, "funding-mode": "staged" };
  const stagedReady = buildDeploymentFundingReadiness({ balance: "2200000" }, stagedOptions);
  assert.equal(stagedReady.funding_ready, true);
  assert.equal(stagedReady.funding_estimate.funding_mode, "staged");
  assert.equal(stagedReady.funding_estimate.aggregate_recommended_min_balance_sun, "11000000");
  assert.equal(stagedReady.funding_estimate.staged_recommended_min_balance_sun, "2200000");
  assert.equal(stagedReady.funding_gap_sun, "0");
  const stagedUnderfunded = buildDeploymentFundingReadiness(
    { balance: "2199999" },
    stagedOptions,
  );
  assert.equal(stagedUnderfunded.funding_ready, false);
  assert.equal(stagedUnderfunded.funding_gap_sun, "1");
});

test("deployment doctor reports ready local prerequisites without exposing deployer secrets", async () => {
  await withTempDir(async (dir) => {
    const secretPath = join(dir, "deployer.secret.json");
    const verifierPath = join(dir, "verifier.json");
    await writeDeployerSecret(secretPath);
    await writeJson(verifierPath, verifierMaterial());

    const report = await buildDeploymentDoctorReport(
      {
        secret: secretPath,
        verifier: verifierPath,
        "require-secret": "true",
        "require-verifier": "true",
        "require-optional-packages": "true",
      },
      {
        resolveNodeModule: (name) => `/mock/node_modules/${name}/index.js`,
        nodeVersion: "20.11.0",
      },
    );

    assert.equal(report.schema, "iroha-sccp-tron-taira-xor-deployment-doctor/v1");
    assert.equal(report.ready, true);
    assert.equal(report.endpoint, "https://api.trongrid.io");
    assert.equal(report.summary.error ?? 0, 0);
    assert.equal(
      report.checks.find((entry) => entry.name === "deployer_secret")?.address_base58,
      deployerAddress.base58,
    );
    assert.equal(
      report.checks.find((entry) => entry.name === "verifier_material")?.network_id_hex,
      TRON_MAINNET_NETWORK_ID_HEX,
    );
    assert.equal(JSON.stringify(report).includes(bytesToHex(privateKey, false)), false);
  });
});

test("deployment doctor rejects non-0600 or symlinked deployer secrets", async () => {
  await withTempDir(async (dir) => {
    const secretPath = join(dir, "permissive-deployer.secret.json");
    const verifierPath = join(dir, "verifier.json");
    await writeJson(secretPath, deployerSecretRecord(), 0o644);
    await writeJson(verifierPath, verifierMaterial());

    const report = await buildDeploymentDoctorReport(
      {
        secret: secretPath,
        verifier: verifierPath,
        "require-secret": "true",
      },
      {
        resolveNodeModule: (name) => `/mock/node_modules/${name}/index.js`,
        nodeVersion: "20.11.0",
      },
    );

    const deployerSecretCheck = report.checks.find(
      (entry) => entry.name === "deployer_secret",
    );
    assert.equal(report.ready, false);
    assert.equal(deployerSecretCheck?.status, "error");
    assert.match(deployerSecretCheck?.error ?? "", /mode must be 0600/u);
    assert.equal(JSON.stringify(report).includes(bytesToHex(privateKey, false)), false);

    await writeJson(secretPath, deployerSecretRecord(), 0o700);
    const executableReport = await buildDeploymentDoctorReport(
      {
        secret: secretPath,
        verifier: verifierPath,
        "require-secret": "true",
      },
      {
        resolveNodeModule: (name) => `/mock/node_modules/${name}/index.js`,
        nodeVersion: "20.11.0",
      },
    );
    assert.match(
      executableReport.checks.find((entry) => entry.name === "deployer_secret")
        ?.error ?? "",
      /mode must be 0600/u,
    );

    const targetSecretPath = join(dir, "target-deployer.secret.json");
    const symlinkSecretPath = join(dir, "linked-deployer.secret.json");
    await writeJson(targetSecretPath, deployerSecretRecord(), 0o600);
    await symlink(targetSecretPath, symlinkSecretPath);
    const symlinkReport = await buildDeploymentDoctorReport(
      {
        secret: symlinkSecretPath,
        verifier: verifierPath,
        "require-secret": "true",
      },
      {
        resolveNodeModule: (name) => `/mock/node_modules/${name}/index.js`,
        nodeVersion: "20.11.0",
      },
    );
    assert.match(
      symlinkReport.checks.find((entry) => entry.name === "deployer_secret")
        ?.error ?? "",
      /symbolic link/u,
    );
  });
});

test("deployment doctor fails closed for unsafe or missing production inputs", async () => {
  await withTempDir(async (dir) => {
    const verifierPath = join(dir, "bad-verifier.json");
    await writeJson(verifierPath, { ...verifierMaterial(), networkId: routeHash("wrong-network") });

    const report = await buildDeploymentDoctorReport(
      {
        endpoint: "http://127.0.0.1:9090",
        secret: join(dir, "missing.secret.json"),
        verifier: verifierPath,
        "require-secret": "true",
        "require-optional-packages": "true",
      },
      {
        resolveNodeModule: () => {
          throw new Error("not installed");
        },
        nodeVersion: "16.20.0",
      },
    );

    assert.equal(report.ready, false);
    const byName = Object.fromEntries(report.checks.map((entry) => [entry.name, entry]));
    assert.equal(byName.node_version.status, "error");
    assert.equal(byName.tron_endpoint.status, "error");
    assert.equal(byName.deployer_secret.status, "error");
    assert.equal(byName.verifier_material.status, "error");
    assert.equal(byName["optional_package:solc"].status, "error");
    assert.equal(byName["optional_package:ethers"].status, "error");
  });
});

test("deployment doctor can check funded deployer account without broadcasting", async () => {
  await withTempDir(async (dir) => {
    const secretPath = join(dir, "deployer.secret.json");
    const verifierPath = join(dir, "verifier.json");
    await writeDeployerSecret(secretPath);
    await writeJson(verifierPath, verifierMaterial());
    const requests = [];

    const report = await buildDeploymentDoctorReport(
      {
        secret: secretPath,
        verifier: verifierPath,
        "check-account": "true",
      },
      {
        resolveNodeModule: (name) => `/mock/node_modules/${name}/index.js`,
        tronPost: async (endpoint, path, body) => {
          requests.push({ endpoint, path, body });
          return { balance: "73600000000" };
        },
      },
    );

    assert.equal(report.ready, true);
    assert.equal(requests.length, 1);
    assert.deepEqual(requests[0], {
      endpoint: "https://api.trongrid.io",
      path: "wallet/getaccount",
      body: { address: deployerAddress.base58, visible: true },
    });
    assert.equal(report.funding_readiness.funding_ready, true);
    assert.equal(
      report.checks.find((entry) => entry.name === "deployer_funding")?.status,
      "ok",
    );
  });
});

test("TAIRA burn-record contract artifact is compiled with forced IVM ZK mode", async () => {
  const contract = await compileTairaBurnRecordContract();
  assert.equal(contract.schema, "iroha-sccp-taira-xor-burn-record-contract/v1");
  assert.equal(contract.route_id, "taira_tron_xor");
  assert.equal(contract.asset_key, "xor");
  assert.match(contract.code_hash, /^[0-9a-f]{64}$/u);
  assert.match(contract.artifact_sha256, /^0x[0-9a-f]{64}$/u);
  assert.equal(contract.execution.executable, "IvmProved");
  assert.equal(contract.execution.force_zk_mode, true);
  assert.equal(contract.execution.entrypoint, "burn_and_record");
  assert.equal(contract.manifest?.features_bitmap, 1);
});

test("route manifest draft binds deployment evidence, verifier material, and TAIRA burn-record contract", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir);
    const liveEvidencePath = join(dir, "live-evidence.json");
    const verifierCodeHash = routeHash("deployed-verifier-code");
    const expectedBindingHash = tronDestinationBindingHash({
      networkId: TRON_MAINNET_NETWORK_ID_HEX,
      verifierAddress: routeAddresses.verifier.base58,
      verifierCodeHash,
      verifierKeyHash: routeHash("verifier-key"),
    });
    const expectedBindingKey = tronDestinationBindingKey({
      networkId: TRON_MAINNET_NETWORK_ID_HEX,
      verifierAddress: routeAddresses.verifier.base58,
      verifierCodeHash,
      verifierKeyHash: routeHash("verifier-key"),
    });
    await writeJson(liveEvidencePath, routeLiveEvidence({ verifierCodeHash }));

    const manifest = await buildTairaXorRouteManifestDraft({
      evidence: evidencePath,
      "taira-contract": contractPath,
      verifier: verifierPath,
      "verifier-code-hash": verifierCodeHash,
      "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      "vk-backend": "halo2/ipa",
      "vk-name": "taira_xor_burn_record_v1",
      "live-evidence": liveEvidencePath,
      "production-ready": "true",
      "live-readback-checked": "true",
      "confirm-mainnet": "taira_tron_xor",
    });

    assert.equal(manifest.schema, "iroha-sccp-taira-xor-route-manifest-draft/v1");
    assert.equal(manifest.routeId, "taira_tron_xor");
    assert.equal(manifest.assetKey, "xor");
    assert.equal(manifest.productionReady, true);
    assert.equal(manifest.tairaXorTokenAddress, routeAddresses.token.base58);
    assert.equal(manifest.tairaXorBridgeAddress, routeAddresses.bridge.base58);
    assert.equal(manifest.sccpTronSourceBridgeAddress, routeAddresses.sourceBridge.base58);
    assert.equal(manifest.tronVerifierAddress, routeAddresses.verifier.base58);
    assert.equal(manifest.destinationRollout.destinationNetworkId, TRON_MAINNET_NETWORK_ID_HEX);
    assert.equal(manifest.destinationRollout.verifierCodeHash, verifierCodeHash);
    assert.equal(manifest.destinationRollout.verifierKeyHash, routeHash("verifier-key"));
    assert.equal(manifest.destinationRollout.destinationBindingHash, expectedBindingHash);
    assert.equal(manifest.destinationRollout.destinationBindingKey, expectedBindingKey);
    assert.equal(manifest.destinationBinding.bindingHash, expectedBindingHash);
    assert.equal(manifest.tairaXorBurnRecord.settlementAssetDefinitionId, "6TEAJqbb8oEPmLncoNiMRbLEK6tw");
    assert.equal(manifest.tairaXorBurnRecord.contractArtifactB64, burnArtifactB64);
    assert.equal(manifest.tairaXorBurnRecord.artifactSha256, burnArtifactSha256);
    assert.deepEqual(manifest.tairaXorBurnRecord.vkRef, {
      backend: "halo2/ipa",
      name: "taira_xor_burn_record_v1",
    });
    assert.equal(manifest.tairaXorBurnRecord.gasLimit, 2_000_000);
    assert.deepEqual(manifest.postDeployLiveEvidence, {
      fullTomlReady: true,
      sourceBridgeConfigHash: routeHash("source-bridge-config"),
      sourceEventTransactionId: routeHash("source-event-transaction"),
      routeCanaryEvidenceHash: routeHash("route-canary-evidence"),
      routeCanaryTransactionId: routeHash("route-canary-transaction"),
      offlineFullTomlSha256: routeHash("offline-full-toml"),
    });
    assert.equal(JSON.stringify(manifest).includes("private_key"), false);
  });
});

test("route manifest draft defaults to disabled and requires production readiness acknowledgements", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir);
    const baseOptions = {
      evidence: evidencePath,
      "taira-contract": contractPath,
      verifier: verifierPath,
      "verifier-code-hash": routeHash("deployed-verifier-code"),
      "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      "vk-backend": "halo2/ipa",
      "vk-name": "taira_xor_burn_record_v1",
    };

    const disabled = await buildTairaXorRouteManifestDraft(baseOptions);
    assert.equal(disabled.productionReady, false);
    assert.match(disabled.disabledReason, /not production-ready/u);

    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "production-ready": "true",
          "live-readback-checked": "true",
        }),
      /confirm-mainnet/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "production-ready": "true",
          "confirm-mainnet": "taira_tron_xor",
        }),
      /live-readback-checked/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "production-ready": "true",
          "live-readback-checked": "true",
          "confirm-mainnet": "taira_tron_xor",
        }),
      /live-evidence/u,
    );
  });
});

test("route manifest draft supports Nile evidence but blocks production readiness", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      evidence: {
        tron_network: "nile",
        network: "tron-nile",
        chain_id_hex: "0xcd8690dc",
        network_id_hex: TRON_NILE_NETWORK_ID_HEX,
      },
    });
    const baseOptions = {
      "tron-network": "nile",
      evidence: evidencePath,
      "taira-contract": contractPath,
      verifier: verifierPath,
      "verifier-code-hash": routeHash("deployed-verifier-code"),
      "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      "vk-backend": "halo2/ipa",
      "vk-name": "taira_xor_burn_record_v1",
    };

    const disabled = await buildTairaXorRouteManifestDraft(baseOptions);
    assert.equal(disabled.productionReady, false);
    assert.equal(disabled.tronNetwork, "nile");
    assert.equal(disabled.chain, "tron-nile");
    assert.equal(disabled.chainIdHex, "0xcd8690dc");
    assert.equal(disabled.networkIdHex, TRON_NILE_NETWORK_ID_HEX);
    assert.equal(disabled.destinationRollout.destinationNetworkId, TRON_NILE_NETWORK_ID_HEX);
    assert.equal(disabled.destinationBinding.networkIdHex, TRON_NILE_NETWORK_ID_HEX);

    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "production-ready": "true",
          "live-readback-checked": "true",
          "confirm-testnet": "nile",
        }),
      /production-ready.*mainnet/u,
    );
  });
});

test("route manifest draft rejects forged or incomplete live evidence", async () => {
  const cases = [
    {
      name: "missing full TOML readiness",
      mutate: (live) => {
        live.full_toml_ready = false;
      },
      error: /full_toml_ready/u,
    },
    {
      name: "source bridge owner drift",
      mutate: (live) => {
        live.source_bridge.source_bridge_owner_base58 = routeAddresses.token.base58;
      },
      error: /source bridge owner/u,
    },
    {
      name: "destination verifier code drift",
      mutate: (live) => {
        live.destination_verifier.destination_verifier_code_hash = routeHash("wrong-code");
      },
      error: /code hash/u,
    },
    {
      name: "destination binding drift",
      mutate: (live) => {
        live.destination_verifier.destination_binding_hash = routeHash("wrong-binding");
      },
      error: /destination binding hash/u,
    },
    {
      name: "route canary failure",
      mutate: (live) => {
        live.route_canary.status = "failed";
      },
      error: /route_canary\.status/u,
    },
    {
      name: "route canary source drift",
      mutate: (live) => {
        live.route_canary.evidence_source = "operator-note";
      },
      error: /evidence_source/u,
    },
    {
      name: "unused route canary message proof",
      mutate: (live) => {
        live.route_canary_transaction.message_proof_used = false;
      },
      error: /message_proof_used/u,
    },
    {
      name: "route canary wrong source domain",
      mutate: (live) => {
        live.route_canary_transaction.source_domain = 5;
      },
      error: /source domain/u,
    },
    {
      name: "route canary wrong target domain",
      mutate: (live) => {
        live.route_canary_transaction.trigger_contract.public_inputs_target_domain = 0;
      },
      error: /target domain/u,
    },
    {
      name: "route canary wrong proof source domain",
      mutate: (live) => {
        live.route_canary_transaction.trigger_contract.proof_source_domain = 5;
      },
      error: /proof source domain/u,
    },
    {
      name: "route canary message id mismatch",
      mutate: (live) => {
        live.route_canary_transaction.trigger_contract.public_inputs_message_id =
          routeHash("wrong-canary-message");
      },
      error: /message id/u,
    },
    {
      name: "route canary commitment root mismatch",
      mutate: (live) => {
        live.route_canary_transaction.trigger_contract.public_inputs_commitment_root =
          routeHash("wrong-canary-commitment");
      },
      error: /commitment root/u,
    },
    {
      name: "route canary statement hash mismatch",
      mutate: (live) => {
        live.route_canary_transaction.trigger_contract.statement_hash =
          routeHash("wrong-canary-statement");
      },
      error: /statement hash/u,
    },
    {
      name: "route canary destination binding mismatch",
      mutate: (live) => {
        live.route_canary_transaction.destination_binding_hash =
          routeHash("wrong-canary-binding");
      },
      error: /destination binding hash/u,
    },
    {
      name: "route canary network mismatch",
      mutate: (live) => {
        live.route_canary_transaction.network_id = routeHash("wrong-network");
      },
      error: /network id/u,
    },
    {
      name: "route canary owner mismatch",
      mutate: (live) => {
        live.route_canary_transaction.trigger_contract.raw_data_owner_matches_transaction = false;
      },
      error: /raw_data_owner_matches_transaction/u,
    },
    {
      name: "route canary signature mismatch",
      mutate: (live) => {
        live.route_canary_transaction.trigger_contract.signature_recovers_to_owner = false;
      },
      error: /signature_recovers_to_owner/u,
    },
    {
      name: "route canary Base58 contract drift",
      mutate: (live) => {
        live.route_canary_transaction.trigger_contract.contract_base58 = routeAddresses.bridge.base58;
      },
      error: /contract_base58/u,
    },
    {
      name: "route canary hex contract drift",
      mutate: (live) => {
        live.route_canary_transaction.trigger_contract.contract_address = routeAddresses.bridge.hex;
      },
      error: /contract_address/u,
    },
    {
      name: "missing source event transaction proof readiness",
      mutate: (live) => {
        delete live.source_event_transaction;
      },
      error: /source_event_transaction/u,
    },
    {
      name: "source event transaction proof blockers",
      mutate: (live) => {
        live.source_event_transaction.source_event_transaction_production_ready = false;
        live.source_event_transaction.source_event_transaction_production_blockers = [
          "witness seal proof required",
        ];
      },
      error: /source_event_transaction_production_ready.*witness seal proof required/u,
    },
    {
      name: "missing source event transaction id",
      mutate: (live) => {
        delete live.source_event_transaction.transaction_id;
      },
      error: /source_event_transaction.*transaction_id/u,
    },
  ];

  for (const { name, mutate, error } of cases) {
    await withTempDir(async (dir) => {
      const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir);
      const liveEvidencePath = join(dir, "live-evidence.json");
      const live = JSON.parse(JSON.stringify(routeLiveEvidence()));
      mutate(live);
      await writeJson(liveEvidencePath, live);

      await assert.rejects(
        () =>
          buildTairaXorRouteManifestDraft({
            evidence: evidencePath,
            "taira-contract": contractPath,
            verifier: verifierPath,
            "verifier-code-hash": routeHash("deployed-verifier-code"),
            "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
            "vk-backend": "halo2/ipa",
            "vk-name": "taira_xor_burn_record_v1",
            "live-evidence": liveEvidencePath,
            "production-ready": "true",
            "live-readback-checked": "true",
            "confirm-mainnet": "taira_tron_xor",
          }),
        error,
        name,
      );
    });
  }
});

test("route manifest draft rejects wrong route, wrong network, and duplicate deployment addresses", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      evidence: { route_id: "minamoto_tron_xor" },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      /route_id/u,
    );
  });
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      evidence: { network_id_hex: routeHash("wrong-network") },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      /tron-mainnet/u,
    );
  });
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      evidence: { sccp_tron_destination_verifier_address: routeAddresses.bridge.base58 },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      /distinct/u,
    );
  });
});

test("route manifest draft rejects forged deployment evidence hashes and stale checklists", async () => {
  const cases = [
    {
      name: "route hash drift",
      evidence: { route_id_hash: routeHash("wrong-route") },
      error: /route_id_hash/u,
    },
    {
      name: "asset hash drift",
      evidence: { asset_key_hash: routeHash("wrong-asset") },
      error: /asset_key_hash/u,
    },
    {
      name: "missing checklist",
      evidence: { required_post_deploy_checks: undefined },
      error: /required_post_deploy_checks/u,
    },
    {
      name: "stale checklist",
      evidence: {
        required_post_deploy_checks: requiredPostDeployChecks.slice(0, -1),
      },
      error: /required_post_deploy_checks is missing/u,
    },
    {
      name: "Base58 value in token hex field",
      evidence: {
        taira_xor_token_address_hex: routeAddresses.token.base58,
      },
      error: /token hex must be a 21-byte TRON hex address/u,
    },
    {
      name: "Solidity value in verifier hex field",
      evidence: {
        sccp_tron_destination_verifier_address_hex:
          routeAddresses.verifier.solidity,
      },
      error: /verifier hex must be a 21-byte TRON hex address/u,
    },
  ];

  for (const { name, evidence, error } of cases) {
    await withTempDir(async (dir) => {
      const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
        evidence,
      });
      await assert.rejects(
        () =>
          buildTairaXorRouteManifestDraft({
            evidence: evidencePath,
            "taira-contract": contractPath,
            verifier: verifierPath,
            "verifier-code-hash": routeHash("deployed-verifier-code"),
            "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
            "vk-backend": "halo2/ipa",
            "vk-name": "taira_xor_burn_record_v1",
          }),
        error,
        name,
      );
    });
  }
});

test("route manifest draft rejects aliases, malformed VK refs, and mismatched verifier hashes", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir);
    const baseOptions = {
      evidence: evidencePath,
      "taira-contract": contractPath,
      verifier: verifierPath,
      "verifier-code-hash": routeHash("deployed-verifier-code"),
      "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      "vk-backend": "halo2/ipa",
      "vk-name": "taira_xor_burn_record_v1",
    };
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "settlement-asset-definition-id": "xor#universal",
        }),
      /canonical Base58 asset definition ID/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "vk-name": "bad name",
        }),
      /unsupported characters/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "verifier-key-hash": routeHash("different-verifier-key"),
        }),
      /does not match --verifier material/u,
    );
  });
});

test("route manifest draft rejects tampered TAIRA burn-record contract artifacts", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      contract: { artifact_sha256: routeHash("wrong-artifact-sha") },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      /artifact_sha256/u,
    );
  });
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      contract: { artifact_b64: "not base64" },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      /strict base64/u,
    );
  });
  await withTempDir(async (dir) => {
    const tinyArtifact = Buffer.from("Nrt0");
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      contract: {
        artifact_b64: tinyArtifact.toString("base64"),
        artifact_sha256: bytesToHex(sha256(tinyArtifact)),
      },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      /decode to 32-/u,
    );
  });
  await withTempDir(async (dir) => {
    const oversizedArtifact = Buffer.alloc(TAIRA_BURN_RECORD_ARTIFACT_MAX_BYTES + 1, 1);
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      contract: {
        artifact_b64: oversizedArtifact.toString("base64"),
        artifact_sha256: bytesToHex(sha256(oversizedArtifact)),
      },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      new RegExp(`-${TAIRA_BURN_RECORD_ARTIFACT_MAX_BYTES} bytes`, "u"),
    );
  });
});

test("route manifest draft rejects destination binding and gas limit mismatches", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir);
    const baseOptions = {
      evidence: evidencePath,
      "taira-contract": contractPath,
      verifier: verifierPath,
      "verifier-code-hash": routeHash("deployed-verifier-code"),
      "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      "vk-backend": "halo2/ipa",
      "vk-name": "taira_xor_burn_record_v1",
    };
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "expected-destination-binding-hash": routeHash("wrong-binding"),
        }),
      /destination binding hash/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "expected-destination-binding-key": "tron:bad",
        }),
      /destination binding key/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "gas-limit": "0",
        }),
      /gas-limit/u,
    );
  });
});
