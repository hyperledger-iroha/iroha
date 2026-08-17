import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
import { parseCanonicalContractAddress } from "../src/contractAddress.js";
import {
  buildCancelSmartContractCodeUploadInstruction,
  buildCommitContractDeploymentInstruction,
  buildFinalizeSmartContractCodeUploadInstruction,
  buildUploadSmartContractCodeChunkInstruction,
} from "../src/instructionBuilders.js";
import { computeIvmArtifactHashes } from "../src/ivmArtifact.js";
import { NetworkId } from "../src/networkId.js";
import {
  noritoDecodeInstruction,
  noritoEncodeInstruction,
} from "../src/norito.js";
import {
  deploySmartContractBrowser,
  deriveContractAddress,
  prepareBrowserContractArtifact,
} from "../src/smartContractDeployment.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import {
  browserTransactionPayloadHashHex,
  browserSignedTransactionHashHex,
  buildBrowserInstructionTransactionPayload,
  finalizeBrowserInstructionTransaction,
  validateBrowserInstructionTransactionSignable,
} from "../src/transactionCodec.js";
import { parseStrictLosslessIntegerJson } from "../src/strictLosslessJson.js";

const CURRENT_ARTIFACT_FIXTURE = parseStrictLosslessIntegerJson(
  readFileSync(
    new URL("./fixtures/current_rust_contract_artifact.json", import.meta.url),
    "utf8",
  ),
  "current Rust contract artifact fixture",
);
const ABI_HASH = CURRENT_ARTIFACT_FIXTURE.artifact_semantics.abi_hash_hex;
const PRIVATE_KEY = Buffer.from(
  "CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
  "hex",
);
const PUBLIC_KEY = Buffer.from(ed25519.getPublicKey(PRIVATE_KEY));
const NETWORK_ID = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const AUTHORITY = AccountAddress.fromAccount({
  algorithm: "ed25519",
  publicKey: PUBLIC_KEY,
}).toI105(753);
const AUTHORITY_FEE_PAYMENT = Object.freeze({
  payer: "authority",
  chargeLimits: Object.freeze([]),
});
function deploymentFixture() {
  return {
    artifactBytes: Buffer.from(CURRENT_ARTIFACT_FIXTURE.artifact_base64, "base64"),
    codeHashHex: CURRENT_ARTIFACT_FIXTURE.artifact_semantics.code_hash_hex,
    manifest: structuredClone(CURRENT_ARTIFACT_FIXTURE.manifest),
  };
}

function hashLiteral(hex) {
  const body = hex.toUpperCase();
  let crc = 0xffff;
  for (const byte of Buffer.from(`hash:${body}`, "utf8")) {
    crc ^= (byte & 0xff) << 8;
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 0x8000) !== 0
          ? ((crc << 1) ^ 0x1021) & 0xffff
          : (crc << 1) & 0xffff;
    }
  }
  return `hash:${body}#${crc.toString(16).toUpperCase().padStart(4, "0")}`;
}

function deploymentState(overrides = {}) {
  return {
    authority: AUTHORITY,
    contract_alias: "demo::universal",
    deploy_nonce: "7",
    dataspace_alias: "universal",
    dataspace_id: "0",
    previous_contract_address: null,
    observed_block_height: "10",
    observed_block_hash: hashLiteral("ab".repeat(32)),
    ledger_time_ms: "123456",
    chain_discriminant: "753",
    ...overrides,
  };
}

test("current smart-contract deployment instructions round-trip through Norito", () => {
  const { codeHashHex } = deploymentFixture();
  const instructions = [
    buildUploadSmartContractCodeChunkInstruction({
      codeHash: codeHashHex,
      totalSize: 4,
      chunkIndex: 0,
      chunkCount: 1,
      chunk: Uint8Array.from([1, 2, 3, 4]),
    }),
    buildFinalizeSmartContractCodeUploadInstruction({
      codeHash: codeHashHex,
      totalSize: 4,
      chunkCount: 1,
    }),
    buildCancelSmartContractCodeUploadInstruction({ codeHash: codeHashHex }),
    buildCommitContractDeploymentInstruction({
      expectedDeployNonce: 7,
      contractAddress: "irohac1qyqqqqqqqqqqqq8y2pcrtkxvkrn5nt74kjjkjcst6kc56qcqa2dqp",
      codeHash: codeHashHex,
      contractAlias: "demo::universal",
      leaseExpiryMs: 123_456,
      expectedPreviousContractAddress: null,
    }),
  ];
  for (const instruction of instructions) {
    assert.deepEqual(
      noritoDecodeInstruction(noritoEncodeInstruction(instruction)),
      instruction,
    );
  }
});

test("artifact preparation verifies the authenticated CNTR envelope before upload", () => {
  const fixture = deploymentFixture();
  const prepared = prepareBrowserContractArtifact({
    artifactBytes: fixture.artifactBytes,
    manifest: fixture.manifest,
    compilerCodeHash: fixture.codeHashHex,
    compilerAbiHash: ABI_HASH,
  });
  assert.equal(prepared.codeHash, fixture.codeHashHex);
  assert.equal(prepared.abiHash, ABI_HASH);
  assert.equal(prepared.chunkCount, 1);
  assert.deepEqual(
    prepared.steps.map(({ kind }) => kind),
    ["upload_chunk", "finalize_upload"],
  );
  fixture.artifactBytes[0] = 0;
  assert.equal(prepared.artifactBytes[0], 0x49);

  const malformed = deploymentFixture();
  malformed.artifactBytes[49] ^= 1;
  const malformedHash = computeIvmArtifactHashes(malformed.artifactBytes).codeHashHex;
  malformed.manifest.code_hash =
    buildCancelSmartContractCodeUploadInstruction({ codeHash: malformedHash })
      .CancelSmartContractCodeUpload.code_hash;
  assert.throws(
    () =>
      prepareBrowserContractArtifact({
        artifactBytes: malformed.artifactBytes,
        manifest: malformed.manifest,
        compilerCodeHash: malformedHash,
        compilerAbiHash: ABI_HASH,
      }),
    /CNTR interface section/u,
  );
});

test("contract-address derivation matches the pinned current-Rust V1 vector", () => {
  assert.equal(
    deriveContractAddress({
      networkId: NETWORK_ID,
      chainDiscriminant: 753,
      authority: AUTHORITY,
      deployNonce: 7,
      dataspaceId: 0,
    }),
    "irohac1qyqqqqqqqqqqqq8y2pcrtkxvkrn5nt74kjjkjcst6kc56qcqa2dqp",
  );
});

test("contract-address derivation separates equal chain names by exact genesis identity", () => {
  const foreignBytes = NETWORK_ID.toBytes();
  foreignBytes[0] ^= 1;
  const firstDeployment = { chainName: "pk3", networkId: NETWORK_ID };
  const secondDeployment = {
    chainName: "pk3",
    networkId: NetworkId.fromBytes(foreignBytes),
  };
  assert.equal(firstDeployment.chainName, secondDeployment.chainName);
  assert.notDeepEqual(
    firstDeployment.networkId.toBytes(),
    secondDeployment.networkId.toBytes(),
  );
  const common = {
    chainDiscriminant: 753,
    authority: AUTHORITY,
    deployNonce: 7,
    dataspaceId: 0,
  };
  assert.notEqual(
    deriveContractAddress({ ...common, networkId: firstDeployment.networkId }),
    deriveContractAddress({ ...common, networkId: secondDeployment.networkId }),
  );
});

test("contract-address derivation rejects retired chainId input", () => {
  assert.throws(
    () =>
      deriveContractAddress({
        networkId: NETWORK_ID,
        chainId: "pk3",
        chainDiscriminant: 753,
        authority: AUTHORITY,
        deployNonce: 7,
        dataspaceId: 0,
      }),
    /unsupported fields: chainId/u,
  );
});

test("contract-address parsing rejects a checksum-valid legacy HRP", () => {
  assert.throws(
    () =>
      parseCanonicalContractAddress(
        "sorac1qyqqqqqqqqqqqq9rdnnncuwseflztqwhmppl0fyvc37w8gqgs6g62",
      ),
    /canonical irohac prefix/u,
  );
});

test("deployment instruction transactions are locally signed and verified", async () => {
  const { codeHashHex } = deploymentFixture();
  const instruction = buildCancelSmartContractCodeUploadInstruction({
    codeHash: codeHashHex,
  });
  const payloadBytes = buildBrowserInstructionTransactionPayload({
    networkId: NETWORK_ID,
    authority: AUTHORITY,
    chainDiscriminant: 753,
    instructions: [instruction],
    feePayment: AUTHORITY_FEE_PAYMENT,
    creationTimeMs: 123_456,
    nonce: 1,
  });
  const signable = validateBrowserInstructionTransactionSignable({
    networkId: NETWORK_ID,
    payloadBytes,
    payloadHashHex: browserTransactionPayloadHashHex(payloadBytes),
    authority: AUTHORITY,
    signingPublicKey: PUBLIC_KEY,
    signatureAlgorithm: "0",
  });
  assert.equal(signable.signatureAlgorithm, "ed25519");
  const signature = Buffer.from(
    ed25519.sign(Buffer.from(signable.payloadHashHex, "hex"), PRIVATE_KEY),
  );
  const finalized = finalizeBrowserInstructionTransaction(
    signable,
    signature,
    PUBLIC_KEY,
  );
  assert.equal(finalized.signedTransaction[0], 1);
  assert.match(finalized.hashHex, /^[0-9a-f]{64}$/u);
  assert.equal(
    browserSignedTransactionHashHex(finalized.signedTransaction),
    finalized.hashHex,
  );

  const tampered = Buffer.from(signature);
  tampered[0] ^= 1;
  assert.throws(
    () =>
      finalizeBrowserInstructionTransaction(signable, tampered, PUBLIC_KEY),
    /signature does not verify/u,
  );

  const expectedReceiptHeaders = {
    "x-iroha-entrypoint-hash": finalized.hashHex,
    "x-iroha-transaction-hash": finalized.hashHex,
    "x-iroha-signed-transaction-hash": finalized.hashHex,
  };
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () =>
      new Response("{}", {
        status: 202,
        headers: {
          "content-type": "application/json",
          ...expectedReceiptHeaders,
          "x-iroha-entrypoint-hash": "00".repeat(32),
        },
      }),
  });
  await assert.rejects(
    client.submitTransaction(finalized.signedTransaction),
    /does not match the locally signed transaction/u,
  );
});

test("browser deployment retains the existing key locally and commits every step", async () => {
  const fixture = deploymentFixture();
  const submissions = [];
  let stateReads = 0;
  const result = await deploySmartContractBrowser({
    artifactBytes: fixture.artifactBytes,
    manifest: fixture.manifest,
    compilerCodeHash: fixture.codeHashHex,
    compilerAbiHash: ABI_HASH,
    networkId: NETWORK_ID,
    chainDiscriminant: 753,
    authority: AUTHORITY,
    feePayment: AUTHORITY_FEE_PAYMENT,
    contractAlias: "demo::universal",
    clock: () => 123_456,
    nonceForStep: (_step, sequence) => sequence + 1,
    readNodeCapabilities(input) {
      assert.deepEqual(input, {
        networkId: NETWORK_ID,
        chainDiscriminant: "753",
      });
      return {
        abi_version: 1,
        data_model_version: 4,
        signed_transaction_schema_hash_hex:
          "7ab5ff9c572efb316deac478f19209c5",
      };
    },
    signManifest(input) {
      assert.equal(Object.hasOwn(input, "privateKey"), false);
      assert.equal(input.payloadBytes.subarray(0, 4).toString("ascii"), "NRT0");
      return ed25519.sign(input.payloadBytes, PRIVATE_KEY);
    },
    sign(input) {
      assert.equal(Object.hasOwn(input, "privateKey"), false);
      return ed25519.sign(input.payloadHashBytes, PRIVATE_KEY);
    },
    submitAndWait(input) {
      assert.equal(Object.hasOwn(input, "privateKey"), false);
      assert.equal(input.signedTransaction[0], 1);
      submissions.push(input);
      return {
        hash: input.hashHex,
        status: { kind: "Applied", block_height: 11 },
        scope: "global",
        resolved_from: "state",
      };
    },
    readDeploymentState(input) {
      stateReads += 1;
      assert.deepEqual(input, {
        authority: AUTHORITY,
        contract_alias: "demo::universal",
      });
      return deploymentState();
    },
  });
  assert.equal(stateReads, 1);
  assert.deepEqual(
    submissions.map(({ step }) => step.kind),
    ["upload_chunk", "finalize_upload", "register_manifest", "commit_deployment"],
  );
  const registeredManifest = submissions.find(
    ({ step }) => step.kind === "register_manifest",
  ).step.instruction.RegisterSmartContractCode.manifest;
  assert.equal(
    registeredManifest.provenance.signer,
    `ed0120${PUBLIC_KEY.toString("hex").toUpperCase()}`,
  );
  assert.match(registeredManifest.provenance.signature, /^[0-9A-F]{128}$/u);
  assert.equal(
    result.contractAddress,
    "irohac1qyqqqqqqqqqqqq8y2pcrtkxvkrn5nt74kjjkjcst6kc56qcqa2dqp",
  );
  assert.equal(result.observedBlockHeight, "10");
  assert.equal(result.observedBlockHash, hashLiteral("ab".repeat(32)));
  assert.equal(result.observedBlockHashHex, "ab".repeat(32));
  assert.equal(result.ledgerTimeMs, "123456");
  assert.equal(Object.hasOwn(result, "artifactAdmission"), false);
  assert.equal(result.transactions.length, 4);
});

test("browser deployment rejects retired pre-release options before callbacks", async () => {
  const fixture = deploymentFixture();
  let externalCalls = 0;
  const options = {
    artifactBytes: fixture.artifactBytes,
    manifest: fixture.manifest,
    compilerCodeHash: fixture.codeHashHex,
    compilerAbiHash: ABI_HASH,
    networkId: NETWORK_ID,
    chainDiscriminant: 753,
    authority: AUTHORITY,
    feePayment: AUTHORITY_FEE_PAYMENT,
    contractAlias: "demo::universal",
    readNodeCapabilities() {
      externalCalls += 1;
      throw new Error("must not read node capabilities");
    },
    readDeploymentState() {
      externalCalls += 1;
      throw new Error("must not read deployment state");
    },
    signManifest() {
      externalCalls += 1;
      throw new Error("must not sign manifest");
    },
    sign() {
      externalCalls += 1;
      throw new Error("must not sign transaction");
    },
    submitAndWait() {
      externalCalls += 1;
      throw new Error("must not submit transaction");
    },
  };
  const retiredVerifierOption = "artifactAdmission" + "Verifier";
  await assert.rejects(
    deploySmartContractBrowser({
      ...options,
      [retiredVerifierOption]: {},
    }),
    /deployment options contains unsupported fields: artifactAdmissionVerifier/u,
  );
  assert.equal(externalCalls, 0);
});

test("browser deployment stops without exact persisted Applied finality and authoritative state", async () => {
  const fixture = deploymentFixture();
  const base = {
    artifactBytes: fixture.artifactBytes,
    manifest: fixture.manifest,
    compilerCodeHash: fixture.codeHashHex,
    compilerAbiHash: ABI_HASH,
    networkId: NETWORK_ID,
    chainDiscriminant: 753,
    authority: AUTHORITY,
    feePayment: AUTHORITY_FEE_PAYMENT,
    contractAlias: "demo::universal",
    clock: () => 123_456,
    nonceForStep: (_step, sequence) => sequence + 1,
    readNodeCapabilities: () => ({
      abi_version: 1,
      data_model_version: 4,
      signed_transaction_schema_hash_hex:
        "7ab5ff9c572efb316deac478f19209c5",
    }),
    signManifest: ({ payloadBytes }) =>
      ed25519.sign(payloadBytes, PRIVATE_KEY),
    sign: ({ payloadHashBytes }) => ed25519.sign(payloadHashBytes, PRIVATE_KEY),
  };
  let submissions = 0;
  await assert.rejects(
    deploySmartContractBrowser({
      ...base,
      submitAndWait() {
        submissions += 1;
        return {
          hash: "00".repeat(32),
          status: { kind: "Applied", block_height: 11 },
          scope: "global",
          resolved_from: "state",
        };
      },
      readDeploymentState: () => deploymentState(),
    }),
    /status hash does not match the submitted transaction/u,
  );
  assert.equal(submissions, 1);

  await assert.rejects(
    deploySmartContractBrowser({
      ...base,
      submitAndWait: ({ hashHex }) => ({
        hash: hashHex,
        status: { kind: "Applied", block_height: 11 },
        scope: "global",
        resolved_from: "state",
      }),
      readDeploymentState: () => ({
        ...deploymentState(),
        signatureVerified: true,
      }),
    }),
    /unsupported fields: signatureVerified/u,
  );
});

test("deployment rejects incompatible node bytes and invalid manifest provenance before upload", async () => {
  const fixture = deploymentFixture();
  const base = {
    artifactBytes: fixture.artifactBytes,
    manifest: fixture.manifest,
    compilerCodeHash: fixture.codeHashHex,
    compilerAbiHash: ABI_HASH,
    networkId: NETWORK_ID,
    chainDiscriminant: 753,
    authority: AUTHORITY,
    feePayment: AUTHORITY_FEE_PAYMENT,
    contractAlias: "demo::universal",
    sign: ({ payloadHashBytes }) => ed25519.sign(payloadHashBytes, PRIVATE_KEY),
    submitAndWait() {
      throw new Error("no deployment bytes may be submitted");
    },
    readDeploymentState: () => deploymentState(),
  };
  let manifestSignCalls = 0;
  await assert.rejects(
    deploySmartContractBrowser({
      ...base,
      readNodeCapabilities: () => ({
        abi_version: 1,
        data_model_version: 4,
        signed_transaction_schema_hash_hex: "00".repeat(16),
      }),
      signManifest() {
        manifestSignCalls += 1;
        throw new Error("manifest must not be signed");
      },
    }),
    /signed_transaction_schema_hash_hex does not match/u,
  );
  assert.equal(manifestSignCalls, 0);

  await assert.rejects(
    deploySmartContractBrowser({
      ...base,
      readNodeCapabilities: () => ({
        abi_version: 1,
        data_model_version: 4,
        signed_transaction_schema_hash_hex:
          "7ab5ff9c572efb316deac478f19209c5",
      }),
      signManifest: () => Buffer.alloc(64, 1),
    }),
    /manifest signature does not verify/u,
  );
});

test("deployment rejects non-Rust aliases and state/address disagreement before signing", async () => {
  const fixture = deploymentFixture();
  let signCalls = 0;
  const base = {
    artifactBytes: fixture.artifactBytes,
    manifest: fixture.manifest,
    compilerCodeHash: fixture.codeHashHex,
    compilerAbiHash: ABI_HASH,
    networkId: NETWORK_ID,
    chainDiscriminant: 753,
    authority: AUTHORITY,
    feePayment: AUTHORITY_FEE_PAYMENT,
    readNodeCapabilities: () => ({
      abi_version: 1,
      data_model_version: 4,
      signed_transaction_schema_hash_hex:
        "7ab5ff9c572efb316deac478f19209c5",
    }),
    signManifest() {
      signCalls += 1;
      throw new Error("manifest signing must not be reached");
    },
    sign() {
      throw new Error("transaction signing must not be reached");
    },
    submitAndWait() {
      throw new Error("submission must not be reached");
    },
  };

  for (const field of ["chain", "chain_id", "chainId"]) {
    await assert.rejects(
      deploySmartContractBrowser({
        ...base,
        [field]: "pk3",
        contractAlias: "demo::universal",
        readDeploymentState: () => deploymentState(),
      }),
      new RegExp(`deployment options contains unsupported fields: ${field}`, "u"),
    );
  }
  for (const networkId of [
    NETWORK_ID.literal,
    NETWORK_ID.toBytes(),
    { literal: NETWORK_ID.literal, toBytes: () => NETWORK_ID.toBytes() },
  ]) {
    await assert.rejects(
      deploySmartContractBrowser({
        ...base,
        networkId,
        contractAlias: "demo::universal",
        readDeploymentState: () => deploymentState(),
      }),
      /deployment options\.networkId must be a NetworkId/u,
    );
  }

  await assert.rejects(
    deploySmartContractBrowser({
      ...base,
      contractAlias: "demo#bad::universal",
      readDeploymentState: () => deploymentState(),
    }),
    /canonical contract-alias syntax/u,
  );

  const wrongDataspaceAddress = deriveContractAddress({
    networkId: NETWORK_ID,
    chainDiscriminant: 753,
    authority: AUTHORITY,
    deployNonce: 6,
    dataspaceId: 1,
  });
  await assert.rejects(
    deploySmartContractBrowser({
      ...base,
      contractAlias: "demo::universal",
      readDeploymentState: () =>
        deploymentState({ previous_contract_address: wrongDataspaceAddress }),
    }),
    /different deployment dataspace/u,
  );
  assert.equal(signCalls, 0);
});
