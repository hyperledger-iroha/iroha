import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
import {
  buildCancelSmartContractCodeUploadInstruction,
  buildCommitContractDeploymentInstruction,
  buildFinalizeSmartContractCodeUploadInstruction,
  buildUploadSmartContractCodeChunkInstruction,
} from "../src/instructionBuilders.js";
import { computeIvmArtifactHashes } from "../src/ivmArtifact.js";
import {
  noritoDecodeInstruction,
  noritoEncodeInstruction,
} from "../src/norito.js";
import {
  deploySmartContractBrowser,
  deriveContractAddress,
  prepareBrowserContractArtifact,
} from "../src/smartContractDeployment.js";
import { createStaticArtifactAdmissionVerifier } from "./helpers/artifactAdmissionWasm.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import {
  browserTransactionPayloadHashHex,
  browserSignedTransactionHashHex,
  buildBrowserInstructionTransactionPayload,
  finalizeBrowserInstructionTransaction,
  validateBrowserInstructionTransactionSignable,
} from "../src/transactionCodec.js";

const CURRENT_ARTIFACT_FIXTURE = JSON.parse(
  readFileSync(
    new URL("./fixtures/current_rust_contract_artifact.json", import.meta.url),
    "utf8",
  ),
);
const ABI_HASH = CURRENT_ARTIFACT_FIXTURE.rust_verifier.abi_hash_hex;
const PRIVATE_KEY = Buffer.from(
  "CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
  "hex",
);
const PUBLIC_KEY = Buffer.from(ed25519.getPublicKey(PRIVATE_KEY));
const AUTHORITY = AccountAddress.fromAccount({
  algorithm: "ed25519",
  publicKey: PUBLIC_KEY,
}).toI105(753);
const ARTIFACT_ADMISSION_VERIFIER = await createStaticArtifactAdmissionVerifier({
  ok: true,
  code_hash_hex: CURRENT_ARTIFACT_FIXTURE.rust_verifier.code_hash_hex,
  abi_hash_hex: CURRENT_ARTIFACT_FIXTURE.rust_verifier.abi_hash_hex,
  header_len: CURRENT_ARTIFACT_FIXTURE.rust_verifier.header_len,
  code_offset: CURRENT_ARTIFACT_FIXTURE.rust_verifier.code_offset,
  entrypoint_count: CURRENT_ARTIFACT_FIXTURE.rust_verifier.entrypoint_count,
  manifest: CURRENT_ARTIFACT_FIXTURE.manifest,
});
function deploymentFixture() {
  return {
    artifactBytes: Buffer.from(CURRENT_ARTIFACT_FIXTURE.artifact_base64, "base64"),
    codeHashHex: CURRENT_ARTIFACT_FIXTURE.rust_verifier.code_hash_hex,
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
      contractAddress: "sorac1qyqqqqqqqqqqqq9rdnnncuwseflztqwhmppl0fyvc37w8gqgs6g62",
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
      chainDiscriminant: 753,
      authority: AUTHORITY,
      deployNonce: 7,
      dataspaceId: 0,
    }),
    "sorac1qyqqqqqqqqqqqq9rdnnncuwseflztqwhmppl0fyvc37w8gqgs6g62",
  );
});

test("deployment instruction transactions are locally signed and verified", async () => {
  const { codeHashHex } = deploymentFixture();
  const instruction = buildCancelSmartContractCodeUploadInstruction({
    codeHash: codeHashHex,
  });
  const payloadBytes = buildBrowserInstructionTransactionPayload({
    chainId: "pk3",
    authority: AUTHORITY,
    chainDiscriminant: 753,
    instructions: [instruction],
    creationTimeMs: 123_456,
    nonce: 1,
  });
  const signable = validateBrowserInstructionTransactionSignable({
    payloadBytes,
    payloadHashHex: browserTransactionPayloadHashHex(payloadBytes),
    authority: AUTHORITY,
    signingPublicKey: PUBLIC_KEY,
    signatureAlgorithm: "ed25519",
  });
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

  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () =>
      new Response("{}", {
        status: 202,
        headers: {
          "content-type": "application/json",
          "x-iroha-entrypoint-hash": hashLiteral("00".repeat(32)),
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
    artifactAdmissionVerifier: ARTIFACT_ADMISSION_VERIFIER,
    artifactBytes: fixture.artifactBytes,
    manifest: fixture.manifest,
    compilerCodeHash: fixture.codeHashHex,
    compilerAbiHash: ABI_HASH,
    chainId: "pk3",
    chainDiscriminant: 753,
    authority: AUTHORITY,
    contractAlias: "demo::universal",
    clock: () => 123_456,
    nonceForStep: (_step, sequence) => sequence + 1,
    readNodeCapabilities(input) {
      assert.deepEqual(input, {
        chainId: "pk3",
        chainDiscriminant: "753",
      });
      return {
        abi_version: 1,
        data_model_version: 1,
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
    "sorac1qyqqqqqqqqqqqq9rdnnncuwseflztqwhmppl0fyvc37w8gqgs6g62",
  );
  assert.equal(result.observedBlockHeight, "10");
  assert.equal(result.observedBlockHash, hashLiteral("ab".repeat(32)));
  assert.equal(result.observedBlockHashHex, "ab".repeat(32));
  assert.equal(result.ledgerTimeMs, "123456");
  assert.deepEqual(result.artifactAdmission, {
    verifierSha256Hex: ARTIFACT_ADMISSION_VERIFIER.verifierSha256Hex,
    headerLength: CURRENT_ARTIFACT_FIXTURE.rust_verifier.header_len,
    codeOffset: CURRENT_ARTIFACT_FIXTURE.rust_verifier.code_offset,
    entrypointCount: CURRENT_ARTIFACT_FIXTURE.rust_verifier.entrypoint_count,
  });
  assert.equal(result.transactions.length, 4);
});

test("browser deployment fails closed without authentic shared artifact admission", async () => {
  const fixture = deploymentFixture();
  let externalCalls = 0;
  const options = {
    artifactBytes: fixture.artifactBytes,
    manifest: fixture.manifest,
    compilerCodeHash: fixture.codeHashHex,
    compilerAbiHash: ABI_HASH,
    chainId: "pk3",
    chainDiscriminant: 753,
    authority: AUTHORITY,
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
  await assert.rejects(
    deploySmartContractBrowser(options),
    /must come from instantiateIvmArtifactAdmissionWasm/u,
  );
  await assert.rejects(
    deploySmartContractBrowser({
      ...options,
      artifactAdmissionVerifier: {
        verifierSha256Hex: "00".repeat(32),
        verify: () => ({ ok: true }),
      },
    }),
    /must come from instantiateIvmArtifactAdmissionWasm/u,
  );
  const rejectingVerifier = await createStaticArtifactAdmissionVerifier({
    ok: false,
    error: "invalid contract artifact: disallowed syscall 0xfe0000 at pc 0",
  });
  const forbiddenArtifact = Buffer.from(fixture.artifactBytes);
  forbiddenArtifact.set(
    [0x00, 0x00, 0xfe, 0x62],
    CURRENT_ARTIFACT_FIXTURE.rust_verifier.code_offset,
  );
  const forbiddenCodeHash =
    computeIvmArtifactHashes(forbiddenArtifact).codeHashHex;
  const forbiddenManifest = structuredClone(fixture.manifest);
  forbiddenManifest.code_hash = hashLiteral(forbiddenCodeHash);
  await assert.rejects(
    deploySmartContractBrowser({
      ...options,
      artifactAdmissionVerifier: rejectingVerifier,
      artifactBytes: forbiddenArtifact,
      compilerCodeHash: forbiddenCodeHash,
      manifest: forbiddenManifest,
    }),
    /shared IVM artifact admission rejected deployment:.*disallowed syscall 0xfe0000/u,
  );
  assert.equal(externalCalls, 0);
});

test("browser deployment stops without exact persisted Applied finality and authoritative state", async () => {
  const fixture = deploymentFixture();
  const base = {
    artifactAdmissionVerifier: ARTIFACT_ADMISSION_VERIFIER,
    artifactBytes: fixture.artifactBytes,
    manifest: fixture.manifest,
    compilerCodeHash: fixture.codeHashHex,
    compilerAbiHash: ABI_HASH,
    chainId: "pk3",
    chainDiscriminant: 753,
    authority: AUTHORITY,
    contractAlias: "demo::universal",
    clock: () => 123_456,
    nonceForStep: (_step, sequence) => sequence + 1,
    readNodeCapabilities: () => ({
      abi_version: 1,
      data_model_version: 1,
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
    artifactAdmissionVerifier: ARTIFACT_ADMISSION_VERIFIER,
    artifactBytes: fixture.artifactBytes,
    manifest: fixture.manifest,
    compilerCodeHash: fixture.codeHashHex,
    compilerAbiHash: ABI_HASH,
    chainId: "pk3",
    chainDiscriminant: 753,
    authority: AUTHORITY,
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
        data_model_version: 1,
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
        data_model_version: 1,
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
    artifactAdmissionVerifier: ARTIFACT_ADMISSION_VERIFIER,
    artifactBytes: fixture.artifactBytes,
    manifest: fixture.manifest,
    compilerCodeHash: fixture.codeHashHex,
    compilerAbiHash: ABI_HASH,
    chainId: "pk3",
    chainDiscriminant: 753,
    authority: AUTHORITY,
    readNodeCapabilities: () => ({
      abi_version: 1,
      data_model_version: 1,
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

  await assert.rejects(
    deploySmartContractBrowser({
      ...base,
      contractAlias: "demo#bad::universal",
      readDeploymentState: () => deploymentState(),
    }),
    /canonical contract-alias syntax/u,
  );

  const wrongDataspaceAddress = deriveContractAddress({
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
