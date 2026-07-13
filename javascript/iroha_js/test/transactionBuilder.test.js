import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import {
  buildRegisterDomainTransaction,
  buildTransaction,
  buildIvmProvedTransaction,
  submitIvmProvedContractCall,
  submitValidationFeeIvmProvedContractCall,
  buildConfidentialTransferProofV2,
  buildConfidentialUnshieldProofV2,
  buildConfidentialUnshieldProofV3,
  buildPrivateCreateKaigiTransaction,
  buildPrivateJoinKaigiTransaction,
  buildPrivateEndKaigiTransaction,
  buildApplySccpRouteGovernanceInstruction,
  buildApplySccpRouteGovernanceTransaction,
  buildPrivateKaigiFeeSpend,
  buildMintAssetTransaction,
  buildMintAndTransferTransaction,
  buildRegisterDomainAndMintTransaction,
  buildRegisterAssetDefinitionAndMintTransaction,
  buildRegisterAssetDefinitionMintAndTransferTransaction,
  buildTransferAssetTransaction,
  buildRegisterRwaTransaction,
  buildTransferRwaTransaction,
  buildSetRwaKeyValueTransaction,
  buildRemoveRwaKeyValueTransaction,
  buildCreateKaigiTransaction,
  buildJoinKaigiTransaction,
  buildRegisterKaigiRelayTransaction,
  buildRegisterSmartContractCodeTransaction,
  buildRegisterSmartContractBytesTransaction,
  buildRemoveSmartContractBytesTransaction,
  buildProposeDeployContractTransaction,
  buildCastZkBallotTransaction,
  buildCastPlainBallotTransaction,
  buildEnactReferendumTransaction,
  buildFinalizeReferendumTransaction,
  buildPersistCouncilForEpochTransaction,
  buildRegisterZkAssetTransaction,
  buildScheduleConfidentialPolicyTransitionTransaction,
  buildCancelConfidentialPolicyTransitionTransaction,
  buildShieldTransaction,
  buildZkTransferTransaction,
  buildUnshieldTransaction,
  buildCreateElectionTransaction,
  buildSubmitBallotTransaction,
  buildFinalizeElectionTransaction,
  hashSignedTransaction,
  hashSignedTransactionPayload,
  hashInstructionBatch,
} from "../src/transaction.js";
import {
  buildBurnAssetInstruction,
  buildMintAssetInstruction,
  buildRegisterDomainInstruction,
  buildSetAccountKeyValueInstruction,
  buildTransferAssetInstruction,
} from "../src/instructionBuilders.js";
import { AccountAddress } from "../src/address.js";
import { ToriiClient } from "../src/toriiClient.js";
import {
  computeIvmArtifactHashes,
  IVM_ARTIFACT_MAX_BYTES,
} from "../src/ivmArtifact.js";
import { makeNativeTest } from "./helpers/native.js";
import {
  VALIDATION_FEE_POLICY_HASH_HEX,
  validationFeePolicyFixture,
} from "./fixtures/validationFeePolicyV1.js";
import {
  NONCANONICAL_VALIDATION_FEE_OVERLAY_BASE64,
  VALIDATION_FEE_BATCH_OVERLAY_BASE64,
  VALIDATION_FEE_DIRECT_OVERLAY_BASE64,
} from "./fixtures/validationFeeOverlayV1.js";

const AUTHORITY_PUBLIC_KEY_HEX =
  "CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
const AUTHORITY_ID = i105FromEd25519PublicKeyHex(AUTHORITY_PUBLIC_KEY_HEX);
const AUTHORITY_ID_INPUT = i105FromEd25519PublicKeyHex(
  AUTHORITY_PUBLIC_KEY_HEX,
);
const PRIVATE_KEY = Buffer.alloc(32, 0x11);
const ZK_IVM_BYTECODE_BASE64 = Buffer.from([
  0x49, 0x56, 0x4d, 0x00,
  0x01, 0x01, 0x01, 0x00,
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  0x01,
  ...Array(32).fill(0),
]).toString("base64");
const {
  codeHashHex: ZK_IVM_CODE_HASH_HEX,
  artifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
} = computeIvmArtifactHashes(Buffer.from(ZK_IVM_BYTECODE_BASE64, "base64"));
const RELAY_PUBLIC_KEY_HEX =
  "641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201";
const RELAY_ACCOUNT_ID = i105FromEd25519PublicKeyHex(RELAY_PUBLIC_KEY_HEX);
const RELAY_ACCOUNT_ID_INPUT =
  i105FromEd25519PublicKeyHex(RELAY_PUBLIC_KEY_HEX);
const ASSET_DEFINITION_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const LILY_ASSET_DEFINITION_ID = "61CtjvNd9T3THAR65GsMVHr82Bjc";
const CANONICAL_ASSET_ID_INPUT = `${ASSET_DEFINITION_ID}#${AUTHORITY_ID}`;
const CANONICAL_LILY_ASSET_ID_INPUT = `${LILY_ASSET_DEFINITION_ID}#${AUTHORITY_ID}`;
const SECOND_CANONICAL_ASSET_ID_INPUT = `${ASSET_DEFINITION_ID}#${RELAY_ACCOUNT_ID}`;
const ASSET_ID = CANONICAL_ASSET_ID_INPUT;
const ASSET_ID_INPUT = CANONICAL_ASSET_ID_INPUT;
const RWA_ID =
  "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities.sora";
const test = makeNativeTest(baseTest);

function i105FromEd25519PublicKeyHex(publicKeyHex) {
  const publicKey = Buffer.from(publicKeyHex.trim(), "hex");
  return AccountAddress.fromAccount({ publicKey }).toI105();
}

function encodeAssetIdForKnownAccount(assetDefinitionId, accountId) {
  assert.equal(assetDefinitionId, ASSET_DEFINITION_ID);
  if (accountId === AUTHORITY_ID || accountId === AUTHORITY_ID_INPUT) {
    return CANONICAL_ASSET_ID_INPUT;
  }
  throw new Error(
    `unexpected account id for test asset encoding: ${accountId}`,
  );
}

function crc16(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let i = 0; i < 8; i += 1) {
      if ((crc & 0x8000) !== 0) {
        crc = ((crc << 1) ^ 0x1021) & 0xffff;
      } else {
        crc = (crc << 1) & 0xffff;
      }
    }
  };

  for (const byte of Buffer.from(tag, "utf8")) {
    processByte(byte);
  }
  processByte(":".charCodeAt(0));
  for (const byte of Buffer.from(body, "utf8")) {
    processByte(byte);
  }
  return crc & 0xffff;
}

function normalizedHashHex(bytes) {
  const buffer = Buffer.from(bytes);
  if (buffer.length !== 32) {
    throw new TypeError("hash literal test helper requires 32 bytes");
  }
  buffer[buffer.length - 1] |= 1;
  const body = buffer.toString("hex").toUpperCase();
  const checksum = crc16("hash", body)
    .toString(16)
    .toUpperCase()
    .padStart(4, "0");
  return `hash:${body}#${checksum}`;
}

function hex32(byte) {
  return `0x${Buffer.alloc(32, byte).toString("hex")}`;
}

function mutateFirstSignedTransactionSignatureByte(signedTransaction) {
  const bytes = Buffer.from(signedTransaction);
  const bareOffset = bytes[0] === 1 ? 1 : 0;

  const readCompactLength = (offset) => {
    let value = 0;
    let shift = 0;
    for (let index = 0; index < 10; index += 1) {
      const byte = bytes[offset + index];
      assert.notEqual(byte, undefined, "compact length must not be truncated");
      value += (byte & 0x7f) * 2 ** shift;
      if ((byte & 0x80) === 0) {
        assert.ok(Number.isSafeInteger(value), "compact length must be safe");
        return { next: offset + index + 1, value };
      }
      shift += 7;
    }
    throw new Error("compact length must terminate within 10 bytes");
  };

  const signatureField = readCompactLength(bareOffset);
  const signaturePayload = readCompactLength(signatureField.next);
  assert.ok(
    signaturePayload.next + signaturePayload.value <= bytes.length,
    "signature payload must fit in the transaction",
  );
  assert.equal(
    bytes.readBigUInt64LE(signaturePayload.next),
    64n,
    "fixture must contain one 64-byte Ed25519 signature",
  );
  const firstSignatureByte = readCompactLength(signaturePayload.next + 8);
  assert.equal(firstSignatureByte.value, 1, "signature byte field must contain one byte");
  bytes[firstSignatureByte.next] ^= 0x80;
  return bytes;
}

function buildSampleSccpRemoveAction() {
  return {
    action: "Remove",
    route: {
      lane_id: {
        source: { network: "bsc_testnet", profile: null },
        target: { network: "sora_taira", profile: null },
      },
      route_id: "taira_bsc_xor",
      asset_key: "xor",
      revision: 1,
    },
  };
}

function toByteArray(bytes) {
  return Array.from(Buffer.from(bytes));
}

function buildSampleRegisterDomain(additionalOptions = {}) {
  return buildRegisterDomainTransaction({
    chainId: "test-chain",
    authority: AUTHORITY_ID_INPUT,
    domainId: "garden_of_live_flowers.sora",
    metadata: { key: "value" },
    creationTimeMs: 1_700_000_000_000,
    ttlMs: 5_000,
    nonce: 42,
    privateKey: PRIVATE_KEY,
    ...additionalOptions,
  });
}

test("buildRegisterDomainTransaction returns canonical hash", () => {
  const built = buildSampleRegisterDomain();
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  assert.ok(Buffer.isBuffer(built.hash));
  assert.equal(built.hash.length, 32);

  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("hashSignedTransactionPayload returns the detached scaffold preimage", () => {
  const first = buildSampleRegisterDomain();
  const signatureMutated = mutateFirstSignedTransactionSignatureByte(
    first.signedTransaction,
  );
  const firstPayloadHash = hashSignedTransactionPayload(
    first.signedTransaction,
    { encoding: "buffer" },
  );
  const secondPayloadHash = hashSignedTransactionPayload(
    signatureMutated,
    { encoding: "buffer" },
  );

  assert.equal(firstPayloadHash.length, 32);
  assert.deepEqual(firstPayloadHash, secondPayloadHash);
  assert.notDeepEqual(
    hashSignedTransaction(first.signedTransaction, { encoding: "buffer" }),
    hashSignedTransaction(signatureMutated, { encoding: "buffer" }),
  );
});

test("hashInstructionBatch binds a settlement batch to its source marker", () => {
  const transfer = buildTransferAssetInstruction({
    sourceAssetHoldingId: CANONICAL_ASSET_ID_INPUT,
    quantity: "2800",
    destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
  });
  const batchFor = (sourceTxHash) => [
    buildSetAccountKeyValueInstruction({
      accountId: AUTHORITY_ID_INPUT,
      key: `pk_cbuae_settlement_${sourceTxHash}`,
      value: {
        protocol: "pk-cbuae-settlement",
        version: 1,
        source_tx_hash: sourceTxHash,
      },
    }),
    transfer,
  ];
  const first = hashInstructionBatch(batchFor("a".repeat(64)), {
    encoding: "buffer",
  });
  const second = hashInstructionBatch(batchFor("b".repeat(64)), {
    encoding: "buffer",
  });

  assert.equal(first.length, 32);
  assert.equal(second.length, 32);
  assert.notDeepEqual(first, second);
});

test("buildRegisterDomainTransaction accepts metadata JSON strings", () => {
  const built = buildSampleRegisterDomain({
    metadata: JSON.stringify({ foo: "bar" }),
  });
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildTransaction normalizes instruction objects", () => {
  const instruction = buildMintAssetInstruction({
    assetId: ASSET_ID_INPUT,
    quantity: "2",
  });
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x01, 0x02]),
    hash: Buffer.alloc(32, 0xaa),
  };

  withNativeBinding(
    {
      buildTransaction: (
        chainId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
        privateKeyAlgorithm,
      ) => {
        captures.push({
          chainId,
          authority,
          instructions,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
          privateKeyAlgorithm,
        });
        return fakeResult;
      },
    },
    () => {
      const built = buildTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        instructions: [instruction],
        metadata: { tag: "value" },
        creationTimeMs: 10,
        ttlMs: 20,
        nonce: 5,
        privateKey: PRIVATE_KEY,
        privateKeyAlgorithm: "secp256k1",
      });
      assert.deepEqual(
        built.signedTransaction,
        Buffer.from(fakeResult.signed_transaction),
      );
      assert.deepEqual(built.hash, Buffer.from(fakeResult.hash));
    },
  );

  assert.equal(captures.length, 1);
  const call = captures[0];
  assert.equal(call.chainId, "test-chain");
  assert.equal(call.authority, AUTHORITY_ID);
  assert.deepEqual(call.instructions, [JSON.stringify(instruction)]);
  assert.equal(call.metadataPayload, JSON.stringify({ tag: "value" }));
  assert.equal(call.creationTimeMs, 10);
  assert.equal(call.ttlMs, 20);
  assert.equal(call.nonce, 5);
  assert.equal(call.privateKeyAlgorithm, "secp256k1");
});

test("buildApplySccpRouteGovernanceInstruction wraps one exact closed action", () => {
  const action = buildSampleSccpRemoveAction();
  assert.deepEqual(
    buildApplySccpRouteGovernanceInstruction(action),
    { ApplySccpRouteGovernance: { action } },
  );
});

test("SCCP route governance action rejects aliases and retired manifests", () => {
  for (const action of [
    { ...buildSampleSccpRemoveAction(), manifest: {} },
    { action: "Remove", route: { ...buildSampleSccpRemoveAction().route, routeId: "alias" } },
    { action: "UpsertManifest", route: {} },
  ]) {
    assert.throws(() => buildApplySccpRouteGovernanceInstruction(action));
  }
});

test("SCCP route governance transaction submits one typed atomic action", () => {
  const action = buildSampleSccpRemoveAction();
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x10, 0x20]),
    hash: Buffer.alloc(32, 0xb1),
  };

  withNativeBinding(
    {
      buildTransaction: (
        chainId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          chainId,
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      const built = buildApplySccpRouteGovernanceTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        action,
        metadata: { op: "apply-sccp-route-governance" },
        creationTimeMs: 1_700_000_000_000,
        ttlMs: 5_000,
        nonce: 7,
        privateKey: PRIVATE_KEY,
      });
      assert.deepEqual(built.hash, Buffer.from(fakeResult.hash));
    },
  );

  assert.equal(captures.length, 1);
  assert.deepEqual(captures[0].instructions, [
    { ApplySccpRouteGovernance: { action } },
  ]);
  assert.deepEqual(JSON.parse(captures[0].metadataPayload), {
    op: "apply-sccp-route-governance",
  });
  assert.equal(captures[0].creationTimeMs, 1_700_000_000_000);
  assert.equal(captures[0].ttlMs, 5_000);
  assert.equal(captures[0].nonce, 7);
  assert.equal(captures[0].secret.equals(PRIVATE_KEY), true);
});

test("transaction helper wrappers forward privateKeyAlgorithm", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x03, 0x04]),
    hash: Buffer.alloc(32, 0xbb),
  };

  withNativeBinding(
    {
      buildTransaction: (
        _chainId,
        _authority,
        _instructions,
        _metadataPayload,
        _creationTimeMs,
        _ttlMs,
        _nonce,
        _secret,
        privateKeyAlgorithm,
      ) => {
        captures.push(privateKeyAlgorithm);
        return fakeResult;
      },
    },
    () => {
      buildMintAssetTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetHoldingId: ASSET_ID_INPUT,
        quantity: "2",
        privateKey: PRIVATE_KEY,
        privateKeyAlgorithm: "secp256k1",
      });
    },
  );

  assert.deepEqual(captures, ["secp256k1"]);
});

test("transaction helper wrappers do not omit privateKeyAlgorithm forwarding", () => {
  const source = readFileSync(
    new URL("../src/transaction.js", import.meta.url),
    "utf8",
  );
  assert.equal(
    /privateKey,\n\s*\}\);/u.test(source),
    false,
    "a buildTransaction call forwards privateKey but not privateKeyAlgorithm",
  );
  assert.equal(
    /privateKey,\n\}\) \{/u.test(source),
    false,
    "a transaction helper accepts privateKey without privateKeyAlgorithm",
  );
});

test("buildTransaction rejects empty instruction arrays", () => {
  assert.throws(
    () =>
      buildTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        instructions: [],
        privateKey: PRIVATE_KEY,
      }),
    /non-empty array/i,
  );
});

 test("buildIvmProvedTransaction normalizes proved executable and attachment", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x03, 0x04]),
    hash: Buffer.alloc(32, 0xbb),
  };
  const proved = {
    bytecode: ZK_IVM_BYTECODE_BASE64,
    overlay: [],
    events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
    gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
  };
  const attachment = {
    backend: "halo2/ipa",
    proof: {
      backend: "halo2/ipa",
      bytes: [1, 2, 3],
    },
    vk_ref: {
      backend: "halo2/ipa",
      name: "ivm-exec-v1",
    },
  };

  withNativeBinding(
    {
      buildIvmProvedTransaction: (
        chainId,
        authority,
        provedPayload,
        attachmentPayload,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          chainId,
          authority,
          provedPayload,
          attachmentPayload,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      const built = buildIvmProvedTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        proved,
        attachment,
        metadata: { gas_limit: 1000 },
        creationTimeMs: 10,
        ttlMs: 20,
        nonce: 5,
        privateKey: PRIVATE_KEY,
      });
      assert.deepEqual(
        built.signedTransaction,
        Buffer.from(fakeResult.signed_transaction),
      );
      assert.deepEqual(built.hash, Buffer.from(fakeResult.hash));
    },
  );

  assert.equal(captures.length, 1);
  const call = captures[0];
  assert.equal(call.chainId, "test-chain");
  assert.equal(call.authority, AUTHORITY_ID);
  assert.deepEqual(JSON.parse(call.provedPayload), proved);
  assert.deepEqual(JSON.parse(call.attachmentPayload), attachment);
  assert.equal(call.metadataPayload, JSON.stringify({ gas_limit: 1000 }));
  assert.equal(call.creationTimeMs, 10);
  assert.equal(call.ttlMs, 20);
  assert.equal(call.nonce, 5);
});

test("buildIvmProvedTransaction rejects empty proved payload strings", () => {
  withNativeBinding(
    {
      buildIvmProvedTransaction: () => {
        throw new Error("native builder should not be called");
      },
    },
    () => {
      assert.throws(
        () =>
          buildIvmProvedTransaction({
            chainId: "test-chain",
            authority: AUTHORITY_ID_INPUT,
            proved: " ",
            attachment: {},
            privateKey: PRIVATE_KEY,
          }),
        /proved must not be an empty JSON string/i,
      );
    },
  );
});

test("submitIvmProvedContractCall rejects code and proof substitution before signing", async (t) => {
  const validProved = {
    bytecode: ZK_IVM_BYTECODE_BASE64,
    overlay: [],
    events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
    gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
  };
  const validAttachment = {
    backend: "halo2/ipa",
    proof: { backend: "halo2/ipa", bytes: [1, 2, 3] },
    vk_ref: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  };
  const headerSubstitutedArtifact = Buffer.from(
    ZK_IVM_BYTECODE_BASE64,
    "base64",
  );
  headerSubstitutedArtifact[16] ^= 0x80;
  const headerSubstitutedBytecode = headerSubstitutedArtifact.toString("base64");
  const bodySubstitutedArtifact = Buffer.concat([
    Buffer.from(ZK_IVM_BYTECODE_BASE64, "base64"),
    Buffer.from([0x80]),
  ]);
  const bodySubstitutedBytecode = bodySubstitutedArtifact.toString("base64");
  const bodySubstitutedArtifactSha256Hex = createHash("sha256")
    .update(bodySubstitutedArtifact)
    .digest("hex");

  async function rejectsBeforeSigning({
    input = {},
    simulationCodeHash = ZK_IVM_CODE_HASH_HEX,
    simulationOverrides = {},
    fetchedBytecode = ZK_IVM_BYTECODE_BASE64,
    fetchedCodeResponse,
    derivedProved = validProved,
    attachment = validAttachment,
    options = {},
    onCodeFetch = () => {},
    onProof = () => {},
    expected,
  }) {
    const calls = {
      simulate: 0,
      fetchCode: 0,
      derive: 0,
      prove: 0,
      sign: 0,
      submit: 0,
    };
    const client = new ToriiClient("https://localhost:8080", {
      fetchImpl: async () => {
        throw new Error("network fetch should be replaced by focused stubs");
      },
    });
    client.simulateContractCall = async () => {
      calls.simulate += 1;
      return {
        ok: true,
        dataspace: "universal",
        contract_address: "tairac1routerfixture",
        code_hash_hex: simulationCodeHash,
        abi_hash_hex: "22".repeat(32),
        entrypoint: "route_swap",
        normalized_payload: null,
        gas_limit: 5000,
        gas_used: 1,
        queued_instructions: [],
        result: null,
        error: null,
        vm_diagnostic: null,
        ...simulationOverrides,
      };
    };
    client.getContractCodeBytes = async (codeHash, fetchOptions) => {
      calls.fetchCode += 1;
      onCodeFetch(codeHash, fetchOptions);
      return fetchedCodeResponse === undefined
        ? { code_b64: fetchedBytecode }
        : fetchedCodeResponse;
    };
    client.deriveIvmProved = async () => {
      calls.derive += 1;
      return { proved: derivedProved };
    };
    client.proveIvmAndWait = async () => {
      calls.prove += 1;
      onProof();
      return {
        job_id: "ab".repeat(16),
        status: "done",
        error: null,
        proved: derivedProved,
        attachment,
      };
    };
    client.submitTransaction = async () => {
      calls.submit += 1;
      throw new Error("transaction must not submit");
    };

    const previous = globalThis.__IROHA_NATIVE_BINDING__;
    globalThis.__IROHA_NATIVE_BINDING__ = {
      buildIvmProvedTransaction: () => {
        calls.sign += 1;
        throw new Error("transaction must not be signed");
      },
    };
    try {
      await assert.rejects(
        () =>
          submitIvmProvedContractCall(
            client,
            {
              chainId: "test-chain",
              authority: AUTHORITY_ID_INPUT,
              privateKey: PRIVATE_KEY,
              vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
              contractAlias: "dlmm_router::dlmm.universal",
              gasLimit: 5000,
              ...input,
            },
            options,
          ),
        expected,
      );
    } finally {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
    assert.equal(calls.sign, 0);
    assert.equal(calls.submit, 0);
    return calls;
  }

  await t.test("rejects invalid wait options before every remote side effect", async () => {
    for (const [options, expected] of [
      [{ proofIntervalMs: -1 }, /proofIntervalMs.*non-negative/i],
      [{ proofTimeoutMs: Number.NaN }, /proofTimeoutMs.*integer/i],
      [{ waitForCommit: "true" }, /waitForCommit must be a boolean/],
      [{ transactionIntervalMs: -1 }, /intervalMs.*non-negative/i],
      [{ transactionTimeoutMs: -1 }, /timeoutMs.*non-negative/i],
      [{ transactionStatusScope: "attacker" }, /scope.*local.*auto.*global/i],
    ]) {
      const calls = await rejectsBeforeSigning({ options, expected });
      assert.equal(calls.simulate, 0);
      assert.equal(calls.fetchCode, 0);
      assert.equal(calls.derive, 0);
      assert.equal(calls.prove, 0);
    }
  });

  await t.test("requires caller-trusted code and full-artifact hashes before simulation", async () => {
    const missing = await rejectsBeforeSigning({
      expected: /expectedCodeHashHex must be exactly 32 hexadecimal bytes/,
    });
    assert.deepEqual(missing, {
      simulate: 0,
      fetchCode: 0,
      derive: 0,
      prove: 0,
      sign: 0,
      submit: 0,
    });
    const duplicate = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expected_code_hash_hex: ZK_IVM_CODE_HASH_HEX,
      },
      expected: /must use exactly one of expectedCodeHashHex, expected_code_hash_hex/,
    });
    assert.equal(duplicate.simulate, 0);

    const missingArtifactHash = await rejectsBeforeSigning({
      input: { expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX },
      expected: /expectedArtifactSha256Hex must be exactly 32 hexadecimal bytes/,
    });
    assert.equal(missingArtifactHash.simulate, 0);
  });

  await t.test("rejects every conflicting input alias before simulation", async () => {
    const trusted = {
      expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
      expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
    };
    const cases = [
      [{ ...trusted, chain_id: "test-chain" }, /exactly one of chainId, chain_id/],
      [{ ...trusted, private_key: PRIVATE_KEY }, /exactly one of privateKey, private_key/],
      [
        {
          ...trusted,
          privateKeyAlgorithm: "ed25519",
          private_key_algorithm: "ed25519",
        },
        /exactly one of privateKeyAlgorithm, private_key_algorithm/,
      ],
      [
        { ...trusted, vk_ref: { backend: "halo2/ipa", name: "ivm-exec-v1" } },
        /exactly one of vkRef, vk_ref/,
      ],
      [
        { ...trusted, contract_alias: "attacker::router.universal" },
        /exactly one of contractAlias, contract_alias/,
      ],
      [
        { ...trusted, contractAddress: "tairac1attacker" },
        /exactly one of contractAddress or contractAlias/,
      ],
      [{ ...trusted, gas_limit: 5000 }, /exactly one of gasLimit, gas_limit/],
      [
        { ...trusted, gasAssetId: null, gas_asset_id: null },
        /exactly one of gasAssetId, gas_asset_id/,
      ],
      [
        { ...trusted, feeSponsor: null, fee_sponsor: null },
        /exactly one of feeSponsor, fee_sponsor/,
      ],
      [
        { ...trusted, creationTimeMs: 1, creation_time_ms: 1 },
        /exactly one of creationTimeMs, creation_time_ms/,
      ],
      [{ ...trusted, ttlMs: 1, ttl_ms: 1 }, /exactly one of ttlMs, ttl_ms/],
      [
        {
          ...trusted,
          requiredOverlayTransfer: null,
          required_overlay_transfer: null,
        },
        /exactly one of requiredOverlayTransfer, required_overlay_transfer/,
      ],
      [
        {
          ...trusted,
          validationFeePolicy: null,
          validation_fee_policy: null,
        },
        /exactly one of validationFeePolicy, validation_fee_policy/,
      ],
      [
        {
          expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
          expected_code_hash_hex: ZK_IVM_CODE_HASH_HEX,
          expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        },
        /exactly one of expectedCodeHashHex, expected_code_hash_hex/,
      ],
      [
        {
          ...trusted,
          expected_artifact_sha256_hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        },
        /exactly one of expectedArtifactSha256Hex, expected_artifact_sha256_hex/,
      ],
    ];
    for (const [input, expected] of cases) {
      const calls = await rejectsBeforeSigning({ input, expected });
      assert.equal(calls.simulate, 0);
      assert.equal(calls.derive, 0);
      assert.equal(calls.prove, 0);
    }
  });

  await t.test("rejects malformed signing, metadata, payload, and timing inputs before simulation", async () => {
    const trusted = {
      expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
      expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
    };
    const circularPayload = {};
    circularPayload.self = circularPayload;
    for (const [input, expected] of [
      [{ ...trusted, privateKey: Buffer.alloc(31) }, /32- or 64-byte Ed25519 key/],
      [{ ...trusted, privateKeyAlgorithm: " ed25519" }, /surrounding whitespace/],
      [{ ...trusted, privateKeyAlgorithm: "attacker" }, /unsupported crypto algorithm/],
      [{ ...trusted, metadata: { contract_address: "attacker" } }, /reserved/],
      [{ ...trusted, payload: circularPayload }, /must be JSON-serializable/],
      [{ ...trusted, gasLimit: 0 }, /positive|greater than zero/i],
      [{ ...trusted, creationTimeMs: -1 }, /non-negative/i],
      [{ ...trusted, ttlMs: Number.NaN }, /integer/i],
      [{ ...trusted, nonce: 0 }, /positive|greater than zero/i],
      [{ ...trusted, nonce: 0x1_0000_0000 }, /fit in u32/],
    ]) {
      const calls = await rejectsBeforeSigning({ input, expected });
      assert.equal(calls.simulate, 0);
      assert.equal(calls.fetchCode, 0);
      assert.equal(calls.derive, 0);
    }
  });

  await t.test("rejects a substituted simulation binding before code fetch", async () => {
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      simulationCodeHash: "11".repeat(32),
      expected: /does not match caller-trusted expected code hash/,
    });
    assert.equal(calls.simulate, 1);
    assert.equal(calls.fetchCode, 0);
    assert.equal(calls.derive, 0);
    assert.equal(calls.prove, 0);
  });

  await t.test("rejects substituted simulation target, entrypoint, and gas before code fetch", async () => {
    const trusted = {
      expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
      expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
    };
    for (const attack of [
      {
        input: {
          ...trusted,
          contractAlias: undefined,
          contractAddress: "tairac1trustedfixture",
        },
        expected: /different contract address than requested/,
      },
      {
        input: { ...trusted, entrypoint: "trusted_entrypoint" },
        expected: /different entrypoint than requested/,
      },
      {
        input: { ...trusted, payload: { amount: "7" } },
        simulationOverrides: { normalized_payload: { amount: "8" } },
        expected: /normalized payload differs from the requested payload/,
      },
      {
        input: trusted,
        simulationOverrides: { gas_limit: 5001 },
        expected: /gas limit 5001 does not match requested gas limit 5000/,
      },
    ]) {
      const calls = await rejectsBeforeSigning(attack);
      assert.equal(calls.simulate, 1);
      assert.equal(calls.fetchCode, 0);
      assert.equal(calls.derive, 0);
    }
  });

  await t.test("rejects header, body, and encoding substitutions before derive", async () => {
    const headerSubstitution = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      fetchedBytecode: headerSubstitutedBytecode,
      expected: /artifact SHA-256 .* does not match caller-trusted expected artifact SHA-256/,
    });
    assert.equal(headerSubstitution.fetchCode, 1);
    assert.equal(headerSubstitution.derive, 0);

    const bodySubstitution = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: bodySubstitutedArtifactSha256Hex,
      },
      fetchedBytecode: bodySubstitutedBytecode,
      expected: /bytecode hash .* does not match expected code hash/,
    });
    assert.equal(bodySubstitution.fetchCode, 1);
    assert.equal(bodySubstitution.derive, 0);

    const nonCanonical = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      fetchedBytecode: `${ZK_IVM_BYTECODE_BASE64}\n`,
      expected: /canonical standard base64/,
    });
    assert.equal(nonCanonical.fetchCode, 1);
    assert.equal(nonCanonical.derive, 0);

    const maxBase64Length = Math.ceil(IVM_ARTIFACT_MAX_BYTES / 3) * 4;
    for (const fetchedBytecode of [
      "A".repeat(maxBase64Length + 1),
      Buffer.alloc(IVM_ARTIFACT_MAX_BYTES + 1).toString("base64"),
    ]) {
      const oversized = await rejectsBeforeSigning({
        input: {
          expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
          expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        },
        fetchedBytecode,
        expected: /exceeds the 4194304-byte artifact limit/,
      });
      assert.equal(oversized.simulate, 1);
      assert.equal(oversized.fetchCode, 1);
      assert.equal(oversized.derive, 0);
      assert.equal(oversized.prove, 0);
      assert.equal(oversized.sign, 0);
      assert.equal(oversized.submit, 0);
    }
  });

  await t.test("forwards the validated AbortSignal to the code-byte fetch", async () => {
    const controller = new AbortController();
    let captured;
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      options: { signal: controller.signal },
      fetchedBytecode: `${ZK_IVM_BYTECODE_BASE64}\n`,
      onCodeFetch(codeHash, fetchOptions) {
        captured = { codeHash, fetchOptions };
      },
      expected: /canonical standard base64/,
    });
    assert.equal(calls.fetchCode, 1);
    assert.equal(captured.codeHash, ZK_IVM_CODE_HASH_HEX);
    assert.equal(captured.fetchOptions.signal, controller.signal);
  });

  await t.test("rejects ambiguous code-byte response shapes before derive", async () => {
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      fetchedCodeResponse: {
        code_b64: ZK_IVM_BYTECODE_BASE64,
        attacker_bytecode: headerSubstitutedBytecode,
      },
      expected: /must contain exactly the code_b64 field/,
    });
    assert.equal(calls.simulate, 1);
    assert.equal(calls.fetchCode, 1);
    assert.equal(calls.derive, 0);
    assert.equal(calls.prove, 0);
    assert.equal(calls.sign, 0);
    assert.equal(calls.submit, 0);
  });

  await t.test("rejects derive bytecode substitution before proving", async () => {
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      derivedProved: { ...validProved, bytecode: headerSubstitutedBytecode },
      expected: /differs from the code-hash-bound deployed contract bytecode/,
    });
    assert.equal(calls.derive, 1);
    assert.equal(calls.prove, 0);
  });

  await t.test("rejects attachment backend and vk_ref substitution after proof", async () => {
    const backend = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      attachment: { ...validAttachment, backend: "stark/fri" },
      expected: /attachment backend differs/,
    });
    assert.equal(backend.prove, 1);

    const proofBackend = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      attachment: {
        ...validAttachment,
        proof: { ...validAttachment.proof, backend: "stark/fri" },
      },
      expected: /attachment proof backend differs/,
    });
    assert.equal(proofBackend.prove, 1);

    const vkRef = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      attachment: {
        ...validAttachment,
        vk_ref: { backend: "halo2/ipa", name: "attacker-key" },
      },
      expected: /attachment vk_ref differs/,
    });
    assert.equal(vkRef.prove, 1);
  });

  await t.test("honors abort after proof before signing or submission", async () => {
    const controller = new AbortController();
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      options: { signal: controller.signal },
      onProof() {
        controller.abort(new Error("abort before final transaction submission"));
      },
      expected: /abort before final transaction submission/,
    });
    assert.equal(calls.prove, 1);
    assert.equal(calls.sign, 0);
    assert.equal(calls.submit, 0);
  });

  await t.test("honors intrinsic abort state despite hostile signal shadows", async () => {
    const controller = new AbortController();
    const calls = await rejectsBeforeSigning({
      input: {
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      },
      options: { signal: controller.signal },
      onProof() {
        controller.abort(new Error("intrinsic abort must prevent signing"));
        Object.defineProperties(controller.signal, {
          aborted: { value: false },
          reason: { value: undefined },
          throwIfAborted: { value() {} },
        });
      },
      expected: /intrinsic abort must prevent signing/,
    });
    assert.equal(calls.prove, 1);
    assert.equal(calls.sign, 0);
    assert.equal(calls.submit, 0);
  });
});

test("submitIvmProvedContractCall snapshots policy inputs, proof-binds, and signs the fee", async () => {
  const policyFixture = validationFeePolicyFixture();
  const requiredOverlayTransfer = {
    sourceAssetHoldingId: `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`,
    quantity: "0.10",
    destinationAccountId: policyFixture.policy.treasury_account_id,
  };
  const expectedTransfer = buildTransferAssetInstruction({
    sourceAssetHoldingId: `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`,
    quantity: "0.10",
    destinationAccountId: policyFixture.policy.treasury_account_id,
  });
  const principalTransfer = buildTransferAssetInstruction({
    sourceAssetHoldingId: `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`,
    quantity: "1.00",
    destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
  });
  const proved = {
    bytecode: ZK_IVM_BYTECODE_BASE64,
    overlay: [principalTransfer, expectedTransfer],
    events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
    gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
  };
  const attachment = {
    backend: "halo2/ipa",
    proof: { backend: "halo2/ipa", bytes: [1, 2, 3] },
    vk_ref: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  };
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should be replaced by focused client stubs");
    },
    validationFeeVerificationContext: policyFixture.verificationContext,
  });
  // The constructor trust anchor is an out-of-band immutable snapshot.
  policyFixture.verificationContext.currentHeight = 100;
  policyFixture.policyRegistry.active_policy_hash = "00".repeat(32);
  policyFixture.governanceKeyset.public_keys_hex[0] = "00".repeat(32);
  const captures = {};
  const submissionController = new AbortController();
  client.simulateContractCall = async (request) => {
    captures.simulationRequest = request;
    // Verification happens before the first await. Mutating every policy field
    // used by the overlay check must not change the verified binding.
    policyFixture.policy.ds_asset_id = CANONICAL_ASSET_ID_INPUT;
    policyFixture.policy.treasury_account_id = RELAY_ACCOUNT_ID_INPUT;
    policyFixture.policy.fee = "99";
    policyFixture.policy.exemption_classes.length = 0;
    return {
      ok: true,
      dataspace: "universal",
      contract_address: "tairac1routerfixture",
      code_hash_hex: ZK_IVM_CODE_HASH_HEX,
      abi_hash_hex: "22".repeat(32),
      entrypoint: "route_swap",
      normalized_payload: { amount: "7" },
      gas_limit: 5000,
      gas_used: 900,
      queued_instructions: [principalTransfer, expectedTransfer],
      result: null,
      error: null,
      vm_diagnostic: null,
    };
  };
  client.getContractCodeBytes = async (codeHash) => {
    captures.codeHash = codeHash;
    return { code_b64: proved.bytecode };
  };
  client.deriveIvmProved = async (request) => {
    captures.deriveRequest = request;
    return { proved };
  };
  client.proveIvmAndWait = async (request, options) => {
    captures.proveRequest = request;
    captures.proveOptions = options;
    return {
      job_id: "ab".repeat(16),
      status: "done",
      error: null,
      proved,
      attachment,
    };
  };
  client.submitTransaction = async (payload, options) => {
    captures.submitted = Buffer.from(payload);
    captures.submitOptions = options;
    return { accepted: true };
  };

  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildIvmProvedTransaction: (
      chainId,
      authority,
      provedPayload,
      attachmentPayload,
      metadataPayload,
      creationTimeMs,
      ttlMs,
      nonce,
      secret,
    ) => {
      captures.signed = {
        chainId,
        authority,
        proved: JSON.parse(provedPayload),
        attachment: JSON.parse(attachmentPayload),
        metadata: JSON.parse(metadataPayload),
        creationTimeMs,
        ttlMs,
        nonce,
        secret: Buffer.from(secret),
      };
      return {
        signed_transaction: Buffer.from([0x03, 0x04]),
        hash: Buffer.alloc(32, 0xbb),
      };
    },
  };
  let result;
  try {
    result = await submitIvmProvedContractCall(
      client,
      {
        chainId: "boi-testnet",
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        entrypoint: "route_swap",
        payload: { amount: "7" },
        gasLimit: 5000,
        metadata: { request_id: "swap-7" },
        validationFeePolicy: {
          signedPolicy: policyFixture.signedPolicy,
          qualifyingTransferCount: 1,
          feeInstructionIndex: 1,
        },
        requiredOverlayTransfer,
      },
      {
        proofIntervalMs: 0,
        proofTimeoutMs: 1000,
        signal: submissionController.signal,
      },
    );
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }

  assert.equal(captures.codeHash, ZK_IVM_CODE_HASH_HEX);
  assert.equal(
    Object.prototype.hasOwnProperty.call(captures.proveRequest, "proved"),
    false,
  );
  assert.equal(captures.proveRequest.bytecode, proved.bytecode);
  assert.deepEqual(captures.deriveRequest.metadata, {
    request_id: "swap-7",
    contract_address: "tairac1routerfixture",
    contract_entrypoint: "route_swap",
    gas_limit: 5000,
    contract_alias: "dlmm_router::dlmm.universal",
    contract_payload: { amount: "7" },
    validation_fee_policy_version: 1,
    validation_fee_policy_hash: VALIDATION_FEE_POLICY_HASH_HEX,
    validation_fee_instruction_index: 1,
  });
  assert.deepEqual(captures.signed.proved, proved);
  assert.deepEqual(captures.signed.attachment, attachment);
  assert.deepEqual(captures.signed.metadata, captures.deriveRequest.metadata);
  assert.deepEqual(captures.submitted, Buffer.from([0x03, 0x04]));
  assert.equal(captures.submitOptions.signal, submissionController.signal);
  assert.equal(result.hash, "bb".repeat(32));
  assert.deepEqual(result.requiredOverlayTransfer, expectedTransfer);
  assert.deepEqual(result.validationFeePolicy, {
    policyVersion: 1,
    policyHash: VALIDATION_FEE_POLICY_HASH_HEX,
    qualifyingTransferCount: 1,
    feeInstructionIndex: 1,
    feeTransferEntryIndex: null,
    feeQuantity: "0.10",
  });
});

test("submitIvmProvedContractCall keeps a 4 MiB proof request below Torii's default cap", async () => {
  const artifact = Buffer.alloc(IVM_ARTIFACT_MAX_BYTES);
  Buffer.from(ZK_IVM_BYTECODE_BASE64, "base64")
    .subarray(0, 17)
    .copy(artifact, 0);
  const hashes = computeIvmArtifactHashes(artifact);
  const bytecode = artifact.toString("base64");
  const proved = {
    bytecode,
    overlay: [],
    events_commitment: "01".repeat(32),
    gas_policy_commitment: "02".repeat(32),
  };
  let proofRequest;
  let signCalls = 0;
  let submitCalls = 0;
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("focused stubs must replace network access");
    },
  });
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "tairac1routerfixture",
    code_hash_hex: hashes.codeHashHex,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: [],
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  client.getContractCodeBytes = async () => ({ code_b64: bytecode });
  client.deriveIvmProved = async () => ({ proved });
  client.proveIvmAndWait = async (request) => {
    proofRequest = request;
    throw new Error("stop after capturing the proof request");
  };
  client.submitTransaction = async () => {
    submitCalls += 1;
    throw new Error("must not submit");
  };
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildIvmProvedTransaction() {
      signCalls += 1;
      throw new Error("must not sign");
    },
  };
  try {
    await assert.rejects(
      () =>
        submitIvmProvedContractCall(client, {
          chainId: "boi-testnet",
          authority: AUTHORITY_ID_INPUT,
          privateKey: PRIVATE_KEY,
          vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
          expectedCodeHashHex: hashes.codeHashHex,
          expectedArtifactSha256Hex: hashes.artifactSha256Hex,
          contractAlias: "dlmm_router::dlmm.universal",
          gasLimit: 5000,
        }),
      /stop after capturing the proof request/,
    );
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }
  assert.equal(Object.hasOwn(proofRequest, "proved"), false);
  assert.equal(proofRequest.bytecode, bytecode);
  assert.ok(
    Buffer.byteLength(JSON.stringify(proofRequest), "utf8") <= 8 * 1024 * 1024,
    "max artifact proof request must fit Torii's default 8 MiB body cap",
  );
  assert.equal(signCalls, 0);
  assert.equal(submitCalls, 0);
});

async function submitBase64ValidationFeeOverlayFixture({
  overlay,
  feeInstructionIndex,
  feeTransferEntryIndex = null,
}) {
  const policyFixture = validationFeePolicyFixture();
  const proved = {
    bytecode: ZK_IVM_BYTECODE_BASE64,
    overlay: [...overlay],
    events_commitment: normalizedHashHex(Buffer.alloc(32, 0x31)),
    gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x32)),
  };
  const attachment = {
    backend: "halo2/ipa",
    proof: { backend: "halo2/ipa", bytes: [7, 8, 9] },
    vk_ref: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  };
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should be replaced by focused client stubs");
    },
    validationFeeVerificationContext: policyFixture.verificationContext,
  });
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "tairac1routerfixture",
    code_hash_hex: ZK_IVM_CODE_HASH_HEX,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: proved.overlay,
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  client.getContractCodeBytes = async () => ({
    code_b64: ZK_IVM_BYTECODE_BASE64,
  });
  client.deriveIvmProved = async () => ({ proved });
  client.proveIvmAndWait = async () => ({
    job_id: "cd".repeat(16),
    status: "done",
    error: null,
    proved,
    attachment,
  });
  client.submitTransaction = async () => ({ accepted: true });

  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  let signedProved;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildIvmProvedTransaction: (_chainId, _authority, provedPayload) => {
      signedProved = JSON.parse(provedPayload);
      return {
        signed_transaction: Buffer.from([0x05, 0x06]),
        hash: Buffer.alloc(32, 0xcc),
      };
    },
  };
  let result;
  try {
    result = await submitValidationFeeIvmProvedContractCall(client, {
      chainId: "boi-testnet",
      authority: AUTHORITY_ID_INPUT,
      privateKey: PRIVATE_KEY,
      vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
      expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
      expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
      contractAlias: "dlmm_router::dlmm.universal",
      gasLimit: 5000,
      validationFeePolicy: {
        signedPolicy: policyFixture.signedPolicy,
        feeInstructionIndex,
        ...(feeTransferEntryIndex === null
          ? {}
          : { feeTransferEntryIndex }),
      },
    });
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }

  return { result, signedProved };
}

test("submitValidationFeeIvmProvedContractCall decodes a real base64 direct overlay", async () => {
  const { result, signedProved } =
    await submitBase64ValidationFeeOverlayFixture({
      overlay: VALIDATION_FEE_DIRECT_OVERLAY_BASE64,
      feeInstructionIndex: 2,
    });

  // The middle transfer also targets treasury, but only the signed coordinate
  // identifies the fee. The middle transfer remains qualifying principal.
  assert.equal(result.validationFeePolicy.qualifyingTransferCount, 2);
  assert.equal(result.validationFeePolicy.feeQuantity, "0.20");
  assert.equal(result.requiredOverlayTransfer.Transfer.Asset.object, "0.20");
  assert.deepEqual(signedProved.overlay, VALIDATION_FEE_DIRECT_OVERLAY_BASE64);
});

test("submitValidationFeeIvmProvedContractCall decodes a real base64 batch overlay", async () => {
  const { result, signedProved } =
    await submitBase64ValidationFeeOverlayFixture({
      overlay: VALIDATION_FEE_BATCH_OVERLAY_BASE64,
      feeInstructionIndex: 0,
      feeTransferEntryIndex: 2,
    });

  assert.equal(result.validationFeePolicy.qualifyingTransferCount, 2);
  assert.equal(result.validationFeePolicy.feeQuantity, "0.20");
  assert.equal(result.validationFeePolicy.feeInstructionIndex, 0);
  assert.equal(result.validationFeePolicy.feeTransferEntryIndex, 2);
  assert.deepEqual(signedProved.overlay, VALIDATION_FEE_BATCH_OVERLAY_BASE64);
});

test("validation-fee submission rejects a legacy archive with noncanonical Numeric scale", async () => {
  await assert.rejects(
    () =>
      submitBase64ValidationFeeOverlayFixture({
        overlay: NONCANONICAL_VALIDATION_FEE_OVERLAY_BASE64,
        feeInstructionIndex: 0,
      }),
    /noncanonical numeric|unique canonical representation/u,
  );
});

test("submitIvmProvedContractCall rejects a missing required transfer before proving or signing", async () => {
  const policyFixture = validationFeePolicyFixture();
  const principalTransfer = buildTransferAssetInstruction({
    sourceAssetHoldingId: `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`,
    quantity: "1.00",
    destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
  });
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should be replaced by focused client stubs");
    },
    validationFeeVerificationContext: policyFixture.verificationContext,
  });
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "tairac1routerfixture",
    code_hash_hex: ZK_IVM_CODE_HASH_HEX,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: [principalTransfer],
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  client.getContractCodeBytes = async () => ({ code_b64: ZK_IVM_BYTECODE_BASE64 });
  client.deriveIvmProved = async () => ({
    proved: {
      bytecode: ZK_IVM_BYTECODE_BASE64,
      overlay: [principalTransfer],
      events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
      gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
    },
  });
  let proveCalled = false;
  let submitCalled = false;
  client.proveIvmAndWait = async () => {
    proveCalled = true;
    throw new Error("proof should not start");
  };
  client.submitTransaction = async () => {
    submitCalled = true;
    throw new Error("transaction should not submit");
  };

  await assert.rejects(
    () =>
      submitIvmProvedContractCall(client, {
        chainId: "boi-testnet",
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        entrypoint: "route_swap",
        gasLimit: 5000,
        validationFeePolicy: {
          signedPolicy: policyFixture.signedPolicy,
          qualifyingTransferCount: 1,
          feeInstructionIndex: 1,
        },
        requiredOverlayTransfer: {
          sourceAssetHoldingId: `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`,
          quantity: "0.10",
          destinationAccountId: policyFixture.policy.treasury_account_id,
        },
      }),
    /does not contain the validation-fee transfer at overlay coordinate 1/,
  );
  assert.equal(proveCalled, false);
  assert.equal(submitCalled, false);
});

test("streamed __proto__ overlay cannot satisfy validation fee or reach signing", async () => {
  const policyFixture = validationFeePolicyFixture();
  const inheritedFee = buildTransferAssetInstruction({
    sourceAssetHoldingId: `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`,
    quantity: "0.10",
    destinationAccountId: policyFixture.policy.treasury_account_id,
  });
  const maliciousInstruction = { Log: { message: "no fee transfer" } };
  Object.defineProperty(maliciousInstruction, "__proto__", {
    value: inheritedFee,
    enumerable: true,
    configurable: true,
    writable: true,
  });
  const proved = {
    bytecode: ZK_IVM_BYTECODE_BASE64,
    overlay: [maliciousInstruction],
    events_commitment: Buffer.alloc(32, 0x41).toString("hex"),
    gas_policy_commitment: Buffer.alloc(32, 0x42).toString("hex"),
  };
  let deriveFetches = 0;
  let proveCalls = 0;
  let signCalls = 0;
  let submitCalls = 0;
  const client = new ToriiClient("https://localhost:8080", {
    validationFeeVerificationContext: policyFixture.verificationContext,
    fetchImpl: async (url) => {
      assert.ok(url.endsWith("/v1/zk/ivm/derive"));
      deriveFetches += 1;
      return new Response(JSON.stringify({ proved }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  });
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "tairac1routerfixture",
    code_hash_hex: ZK_IVM_CODE_HASH_HEX,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: [],
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  client.getContractCodeBytes = async () => ({
    code_b64: ZK_IVM_BYTECODE_BASE64,
  });
  client.proveIvmAndWait = async () => {
    proveCalls += 1;
    throw new Error("proof must not start");
  };
  client.submitTransaction = async () => {
    submitCalls += 1;
    throw new Error("submit must not run");
  };
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildIvmProvedTransaction() {
      signCalls += 1;
      throw new Error("signer must not run");
    },
  };
  try {
    await assert.rejects(
      () =>
        submitIvmProvedContractCall(client, {
          chainId: "boi-testnet",
          authority: AUTHORITY_ID_INPUT,
          privateKey: PRIVATE_KEY,
          vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
          expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
          expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
          contractAlias: "dlmm_router::dlmm.universal",
          gasLimit: 5000,
          validationFeePolicy: {
            signedPolicy: policyFixture.signedPolicy,
            qualifyingTransferCount: 1,
            feeInstructionIndex: 0,
          },
        }),
      /validation-fee submission fails closed on other instruction families/,
    );
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }
  assert.equal(deriveFetches, 1);
  assert.equal(proveCalls, 0);
  assert.equal(signCalls, 0);
  assert.equal(submitCalls, 0);
});

async function assertAmbiguousValidationFeeInstructionStopsSubmission(
  maliciousInstruction,
  {
    feeTransferEntryIndex = null,
    expected = /validation-fee submission fails closed on other instruction families/,
  } = {},
) {
  const policyFixture = validationFeePolicyFixture();
  const proved = {
    bytecode: ZK_IVM_BYTECODE_BASE64,
    overlay: [maliciousInstruction],
    events_commitment: Buffer.alloc(32, 0x51).toString("hex"),
    gas_policy_commitment: Buffer.alloc(32, 0x52).toString("hex"),
  };
  const attachment = {
    backend: "halo2/ipa",
    proof: { backend: "halo2/ipa", bytes: [1] },
    vk_ref: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  };
  const calls = { derive: 0, prove: 0, sign: 0, submit: 0 };
  const client = new ToriiClient("https://localhost:8080", {
    validationFeeVerificationContext: policyFixture.verificationContext,
    fetchImpl: async (url) => {
      assert.ok(url.endsWith("/v1/zk/ivm/derive"));
      calls.derive += 1;
      return new Response(JSON.stringify({ proved }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  });
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "tairac1routerfixture",
    code_hash_hex: ZK_IVM_CODE_HASH_HEX,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: [],
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  client.getContractCodeBytes = async () => ({
    code_b64: ZK_IVM_BYTECODE_BASE64,
  });
  client.proveIvmAndWait = async () => {
    calls.prove += 1;
    return {
      job_id: "ab".repeat(16),
      status: "done",
      proved,
      attachment,
    };
  };
  client.submitTransaction = async () => {
    calls.submit += 1;
    return { accepted: true };
  };

  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = {
    buildIvmProvedTransaction() {
      calls.sign += 1;
      return {
        signed_transaction: Buffer.from([1]),
        hash: Buffer.alloc(32, 1),
      };
    },
  };
  try {
    await assert.rejects(
      () =>
        submitValidationFeeIvmProvedContractCall(client, {
          chainId: "boi-testnet",
          authority: AUTHORITY_ID_INPUT,
          privateKey: PRIVATE_KEY,
          vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
          expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
          expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
          contractAlias: "dlmm_router::dlmm.universal",
          gasLimit: 5000,
          validationFeePolicy: {
            signedPolicy: policyFixture.signedPolicy,
            qualifyingTransferCount: 1,
            feeInstructionIndex: 0,
            ...(feeTransferEntryIndex === null
              ? {}
              : { feeTransferEntryIndex }),
          },
        }),
      expected,
    );
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }
  assert.deepEqual(calls, { derive: 1, prove: 0, sign: 0, submit: 0 });
}

test("validation fee rejects Transfer plus a second top-level variant before side effects", async () => {
  const policyFixture = validationFeePolicyFixture();
  const source = `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`;
  const fee = buildTransferAssetInstruction({
    sourceAssetHoldingId: source,
    quantity: "0.10",
    destinationAccountId: policyFixture.policy.treasury_account_id,
  });
  for (const extraVariant of [
    buildMintAssetInstruction({ assetHoldingId: source, quantity: "1.00" }),
    { Log: { message: "smuggled alongside fee" } },
  ]) {
    await assertAmbiguousValidationFeeInstructionStopsSubmission({
      ...fee,
      ...extraVariant,
    });
  }
});

test("validation fee rejects Asset plus Nft or extra Asset fields before side effects", async () => {
  const policyFixture = validationFeePolicyFixture();
  const source = `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`;
  const fee = buildTransferAssetInstruction({
    sourceAssetHoldingId: source,
    quantity: "0.10",
    destinationAccountId: policyFixture.policy.treasury_account_id,
  });
  await assertAmbiguousValidationFeeInstructionStopsSubmission({
    Transfer: {
      Asset: { ...fee.Transfer.Asset },
      Nft: {
        source: AUTHORITY_ID_INPUT,
        object: "nft$universal",
        destination: policyFixture.policy.treasury_account_id,
      },
    },
  });
  await assertAmbiguousValidationFeeInstructionStopsSubmission({
    Transfer: {
      Asset: { ...fee.Transfer.Asset, attacker_controlled: true },
    },
  });
});

test("validation fee rejects ambiguous transfer batches before side effects", async () => {
  const policyFixture = validationFeePolicyFixture();
  const entry = {
    from: AUTHORITY_ID_INPUT,
    to: policyFixture.policy.treasury_account_id,
    asset_definition: policyFixture.policy.ds_asset_id,
    amount: "0.10",
  };
  await assertAmbiguousValidationFeeInstructionStopsSubmission(
    {
      TransferAssetBatch: {
        entries: [entry],
        attacker_controlled: true,
      },
    },
    {
      feeTransferEntryIndex: 0,
      expected: /TransferAssetBatch must contain exactly entries/,
    },
  );
  await assertAmbiguousValidationFeeInstructionStopsSubmission(
    {
      TransferAssetBatch: {
        entries: [{ ...entry, attacker_controlled: true }],
      },
    },
    {
      feeTransferEntryIndex: 0,
      expected: /entries\[0\] must contain exactly from, to, asset_definition, and amount/,
    },
  );
});

test("validation fee rejects ambiguous recursive multisig proposals before side effects", async () => {
  const policyFixture = validationFeePolicyFixture();
  const fee = buildTransferAssetInstruction({
    sourceAssetHoldingId: `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`,
    quantity: "0.10",
    destinationAccountId: policyFixture.policy.treasury_account_id,
  });
  await assertAmbiguousValidationFeeInstructionStopsSubmission(
    {
      Custom: {
        payload: {
          Propose: {
            account: AUTHORITY_ID_INPUT,
            instructions: [fee],
          },
          Execute: { proposal_id: "attacker" },
        },
      },
    },
    { expected: /Custom\.payload must contain exactly Propose/ },
  );
  await assertAmbiguousValidationFeeInstructionStopsSubmission(
    {
      MultisigPropose: {
        account: AUTHORITY_ID_INPUT,
        instructions: [fee],
        attacker_controlled: true,
      },
    },
    { expected: /MultisigPropose must contain exactly account and instructions/ },
  );
});

test("submitIvmProvedContractCall preserves generic non-policy overlay assertions", async () => {
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should not run");
    },
  });
  let simulationCalled = false;
  client.simulateContractCall = async () => {
    simulationCalled = true;
    return {
      ok: true,
      dataspace: "universal",
      contract_address: "tairac1routerfixture",
      code_hash_hex: ZK_IVM_CODE_HASH_HEX,
      abi_hash_hex: "22".repeat(32),
      entrypoint: "generic_route",
      normalized_payload: null,
      gas_limit: 5000,
      gas_used: 1,
      queued_instructions: [],
      result: null,
      error: null,
      vm_diagnostic: null,
    };
  };
  client.getContractCodeBytes = async () => ({
    code_b64: ZK_IVM_BYTECODE_BASE64,
  });
  client.deriveIvmProved = async () => ({
    proved: {
      bytecode: ZK_IVM_BYTECODE_BASE64,
      overlay: [],
      events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
      gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
    },
  });
  let proveCalled = false;
  client.proveIvmAndWait = async () => {
    proveCalled = true;
    throw new Error("proof should not start");
  };
  await assert.rejects(
    () =>
      submitIvmProvedContractCall(client, {
        chainId: "boi-testnet",
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        gasLimit: 5000,
        requiredOverlayTransfer: {
          sourceAssetHoldingId: CANONICAL_ASSET_ID_INPUT,
          quantity: "0.10",
          destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
        },
      }),
    /must contain the required overlay transfer exactly once \(found 0\)/,
  );
  assert.equal(simulationCalled, true);
  assert.equal(proveCalled, false);
});

test("submitValidationFeeIvmProvedContractCall requires exactly one policy intent", async () => {
  const fixture = validationFeePolicyFixture();
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should not run");
    },
    validationFeeVerificationContext: fixture.verificationContext,
  });
  let simulationCalled = false;
  client.simulateContractCall = async () => {
    simulationCalled = true;
    throw new Error("simulation should not start");
  };
  const baseInput = {
    chainId: "boi-testnet",
    authority: AUTHORITY_ID_INPUT,
    privateKey: PRIVATE_KEY,
    vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
    expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
    expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
    contractAlias: "dlmm_router::dlmm.universal",
    gasLimit: 5000,
  };

  assert.throws(
    () => submitValidationFeeIvmProvedContractCall(client, baseInput),
    /requires validationFeePolicy/,
  );
  assert.throws(
    () =>
      submitValidationFeeIvmProvedContractCall(client, {
        ...baseInput,
        validationFeePolicy: {
          signedPolicy: fixture.signedPolicy,
          feeInstructionIndex: 1,
        },
        validation_fee_policy: {
          signedPolicy: fixture.signedPolicy,
          feeInstructionIndex: 1,
        },
      }),
    /must use exactly one of validationFeePolicy, validation_fee_policy/,
  );
  const untrustedClient = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should not run");
    },
  });
  untrustedClient.validationFeeVerificationContext = fixture.verificationContext;
  untrustedClient._validationFeeVerificationContext = fixture.verificationContext;
  await assert.rejects(
    () =>
      submitValidationFeeIvmProvedContractCall(untrustedClient, {
        ...baseInput,
        validationFeePolicy: {
          signedPolicy: fixture.signedPolicy,
          feeInstructionIndex: 1,
        },
      }),
    /requires ToriiClient\.options\.validationFeeVerificationContext/,
  );
  await assert.rejects(
    () =>
      submitValidationFeeIvmProvedContractCall(client, {
        ...baseInput,
        validationFeePolicy: {
          signedPolicy: fixture.signedPolicy,
          // Even a self-consistent per-call anchor cannot replace constructor
          // trust. This closes the fake-registry/fake-keyset bypass.
          verificationContext: fixture.verificationContext,
          feeInstructionIndex: 1,
        },
      }),
    /cannot override the ToriiClient trusted validation-fee verification context/,
  );
  assert.equal(simulationCalled, false);
});

test("submitIvmProvedContractCall rejects caller fee assertions that conflict with signed policy", async () => {
  const policyFixture = validationFeePolicyFixture();
  const source = `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`;
  const principal = buildTransferAssetInstruction({
    sourceAssetHoldingId: source,
    quantity: "1.00",
    destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
  });
  const fee = buildTransferAssetInstruction({
    sourceAssetHoldingId: source,
    quantity: "0.10",
    destinationAccountId: policyFixture.policy.treasury_account_id,
  });
  const overlay = [principal, fee];
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should not run");
    },
    validationFeeVerificationContext: policyFixture.verificationContext,
  });
  let simulationCalled = false;
  client.simulateContractCall = async () => {
    simulationCalled = true;
    return {
      ok: true,
      dataspace: "universal",
      contract_address: "tairac1routerfixture",
      code_hash_hex: ZK_IVM_CODE_HASH_HEX,
      abi_hash_hex: "22".repeat(32),
      entrypoint: "route_swap",
      normalized_payload: null,
      gas_limit: 5000,
      gas_used: 1,
      queued_instructions: overlay,
      result: null,
      error: null,
      vm_diagnostic: null,
    };
  };
  client.getContractCodeBytes = async () => ({
    code_b64: ZK_IVM_BYTECODE_BASE64,
  });
  client.deriveIvmProved = async () => ({
    proved: {
      bytecode: ZK_IVM_BYTECODE_BASE64,
      overlay,
      events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
      gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
    },
  });
  let proveCalled = false;
  client.proveIvmAndWait = async () => {
    proveCalled = true;
    throw new Error("proof should not start");
  };
  await assert.rejects(
    () =>
      submitIvmProvedContractCall(client, {
        chainId: "boi-testnet",
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        gasLimit: 5000,
        validationFeePolicy: {
          signedPolicy: policyFixture.signedPolicy,
          qualifyingTransferCount: 1,
          feeInstructionIndex: 1,
        },
        requiredOverlayTransfer: {
          sourceAssetHoldingId: `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`,
          quantity: "0.11",
          destinationAccountId: policyFixture.policy.treasury_account_id,
        },
      }),
    /conflicts with the verified validation-fee policy/,
  );
  assert.equal(simulationCalled, true);
  assert.equal(proveCalled, false);
});

test("submitIvmProvedContractCall rejects caller validation-fee metadata", async () => {
  const policyFixture = validationFeePolicyFixture();
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should be replaced by focused client stubs");
    },
    validationFeeVerificationContext: policyFixture.verificationContext,
  });
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "tairac1routerfixture",
    code_hash_hex: ZK_IVM_CODE_HASH_HEX,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: [],
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  client.getContractCodeBytes = async () => ({ code_b64: ZK_IVM_BYTECODE_BASE64 });
  let deriveCalled = false;
  client.deriveIvmProved = async () => {
    deriveCalled = true;
    throw new Error("derive should not start");
  };
  await assert.rejects(
    () =>
      submitIvmProvedContractCall(client, {
        chainId: "boi-testnet",
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        gasLimit: 5000,
        metadata: { validation_fee_policy_hash: "00".repeat(32) },
        validationFeePolicy: {
          signedPolicy: policyFixture.signedPolicy,
          qualifyingTransferCount: 1,
          feeInstructionIndex: 1,
        },
      }),
    /metadata\.validation_fee_policy_hash is reserved/,
  );
  assert.equal(deriveCalled, false);
});

test("submitIvmProvedContractCall derives counts by coordinate and rejects unsupported contexts", async () => {
  const policyFixture = validationFeePolicyFixture();
  const source = `${policyFixture.policy.ds_asset_id}#${AUTHORITY_ID_INPUT}`;
  const principal = buildTransferAssetInstruction({
    sourceAssetHoldingId: source,
    quantity: "1.00",
    destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
  });
  const secondPrincipal = buildTransferAssetInstruction({
    sourceAssetHoldingId: source,
    quantity: "2.00",
    destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
  });
  const overScaledPrincipal = buildTransferAssetInstruction({
    sourceAssetHoldingId: source,
    quantity: "1.000",
    destinationAccountId: RELAY_ACCOUNT_ID_INPUT,
  });
  const fee = buildTransferAssetInstruction({
    sourceAssetHoldingId: source,
    quantity: "0.10",
    destinationAccountId: policyFixture.policy.treasury_account_id,
  });
  const overScaledFee = buildTransferAssetInstruction({
    sourceAssetHoldingId: source,
    quantity: "0.100",
    destinationAccountId: policyFixture.policy.treasury_account_id,
  });

  async function rejectsOverlay(overlay, feeInstructionIndex, expected) {
    const client = new ToriiClient("https://localhost:8080", {
      fetchImpl: async () => {
        throw new Error("network fetch should be replaced by focused client stubs");
      },
      validationFeeVerificationContext: policyFixture.verificationContext,
    });
    client.simulateContractCall = async () => ({
      ok: true,
      dataspace: "universal",
      contract_address: "tairac1routerfixture",
      code_hash_hex: ZK_IVM_CODE_HASH_HEX,
      abi_hash_hex: "22".repeat(32),
      entrypoint: "route_swap",
      normalized_payload: null,
      gas_limit: 5000,
      gas_used: 1,
      queued_instructions: overlay,
      result: null,
      error: null,
      vm_diagnostic: null,
    });
    client.getContractCodeBytes = async () => ({
      code_b64: ZK_IVM_BYTECODE_BASE64,
    });
    client.deriveIvmProved = async () => ({
      proved: {
        bytecode: ZK_IVM_BYTECODE_BASE64,
        overlay,
        events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
        gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
      },
    });
    let proveCalled = false;
    client.proveIvmAndWait = async () => {
      proveCalled = true;
      throw new Error("proof should not start");
    };
    await assert.rejects(
      () =>
        submitIvmProvedContractCall(client, {
          chainId: "boi-testnet",
          authority: AUTHORITY_ID_INPUT,
          privateKey: PRIVATE_KEY,
          vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
          expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
          expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
          contractAlias: "dlmm_router::dlmm.universal",
          gasLimit: 5000,
          validationFeePolicy: {
            signedPolicy: policyFixture.signedPolicy,
            qualifyingTransferCount: 1,
            feeInstructionIndex,
          },
        }),
      expected,
    );
    assert.equal(proveCalled, false);
  }

  await rejectsOverlay(
    [principal, fee, fee],
    1,
    /contains 2 qualifying DS transfers but validationFeePolicy declares 1/,
  );
  await rejectsOverlay(
    [principal, secondPrincipal, fee],
    2,
    /contains 2 qualifying DS transfers but validationFeePolicy declares 1/,
  );
  await rejectsOverlay(
    [principal, overScaledFee],
    1,
    /DS transfer 0:1 uses scale 3, above policy scale 2/,
  );
  await rejectsOverlay(
    [overScaledPrincipal, fee],
    1,
    /DS transfer 0:0 uses scale 3, above policy scale 2/,
  );
  for (const unsupportedInstruction of [
    buildMintAssetInstruction({ assetHoldingId: source, quantity: "1.00" }),
    buildBurnAssetInstruction({ assetHoldingId: source, quantity: "1.00" }),
    { Unregister: { Account: AUTHORITY_ID_INPUT } },
    {
      Transfer: {
        Nft: {
          source: AUTHORITY_ID_INPUT,
          object: "nft$universal",
          destination: RELAY_ACCOUNT_ID_INPUT,
        },
      },
    },
    { Rwa: { Redeem: { id: "attacker$rwa", quantity: "1" } } },
    { Repo: { asset_legs: [{ asset_id: source, quantity: "1" }] } },
    { Settlement: { asset_legs: [{ asset_id: source, quantity: "1" }] } },
  ]) {
    await rejectsOverlay(
      [principal, fee, unsupportedInstruction],
      1,
      /validation-fee submission fails closed on other instruction families/,
    );
  }
  await rejectsOverlay(
    [principal, fee, { FutureNativeInstruction: { attacker_controlled: true } }],
    1,
    /validation-fee submission fails closed on other instruction families/,
  );
  await rejectsOverlay(
    [
      fee,
      principal,
      {
        MultisigPropose: {
          account: AUTHORITY_ID_INPUT,
          instructions: [fee],
        },
      },
    ],
    0,
    /validation-fee coordinate 0 is ambiguous across execution contexts/,
  );
  await rejectsOverlay(
    [
      principal,
      fee,
      {
        MultisigPropose: {
          account: AUTHORITY_ID_INPUT,
          instructions: [principal],
        },
      },
    ],
    1,
    /unsupported nested multisig DS transfer context/,
  );
});

test("submitIvmProvedContractCall rejects conventional non-ZK deployed bytecode", async () => {
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should be replaced by focused client stubs");
    },
  });
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "tairac1routerfixture",
    code_hash_hex: ZK_IVM_CODE_HASH_HEX,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: [],
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  const nonZkBytecode = Buffer.from(ZK_IVM_BYTECODE_BASE64, "base64");
  nonZkBytecode[6] = 0;
  client.getContractCodeBytes = async () => ({
    code_b64: nonZkBytecode.toString("base64"),
  });
  let deriveCalled = false;
  client.deriveIvmProved = async () => {
    deriveCalled = true;
    throw new Error("derive should not start");
  };

  await assert.rejects(
    () =>
      submitIvmProvedContractCall(client, {
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        entrypoint: "route_swap",
        gasLimit: 5000,
      }),
    /not ZK mode/,
  );
  assert.equal(deriveCalled, false);
});

test("submitIvmProvedContractCall rejects a prover payload that differs from derivation", async () => {
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("network fetch should be replaced by focused client stubs");
    },
  });
  const derived = {
    bytecode: ZK_IVM_BYTECODE_BASE64,
    overlay: [],
    events_commitment: normalizedHashHex(Buffer.alloc(32, 0x01)),
    gas_policy_commitment: normalizedHashHex(Buffer.alloc(32, 0x02)),
  };
  client.simulateContractCall = async () => ({
    ok: true,
    dataspace: "universal",
    contract_address: "tairac1routerfixture",
    code_hash_hex: ZK_IVM_CODE_HASH_HEX,
    abi_hash_hex: "22".repeat(32),
    entrypoint: "route_swap",
    normalized_payload: null,
    gas_limit: 5000,
    gas_used: 1,
    queued_instructions: [],
    result: null,
    error: null,
    vm_diagnostic: null,
  });
  client.getContractCodeBytes = async () => ({ code_b64: derived.bytecode });
  client.deriveIvmProved = async () => ({ proved: derived });
  client.proveIvmAndWait = async () => ({
    job_id: "ab".repeat(16),
    status: "done",
    error: null,
    proved: {
      ...derived,
      events_commitment: normalizedHashHex(Buffer.alloc(32, 0x03)),
    },
    attachment: { backend: "halo2/ipa" },
  });
  let submitCalled = false;
  client.submitTransaction = async () => {
    submitCalled = true;
    throw new Error("transaction should not submit");
  };

  await assert.rejects(
    () =>
      submitIvmProvedContractCall(client, {
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        privateKey: PRIVATE_KEY,
        vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
        expectedCodeHashHex: ZK_IVM_CODE_HASH_HEX,
        expectedArtifactSha256Hex: ZK_IVM_ARTIFACT_SHA256_HEX,
        contractAlias: "dlmm_router::dlmm.universal",
        entrypoint: "route_swap",
        gasLimit: 5000,
      }),
    /different from the authoritative derived payload/,
  );
  assert.equal(submitCalled, false);
});

test("buildMintAssetTransaction returns canonical hash", () => {
  const built = buildMintAssetTransaction({
    chainId: "test-chain",
    authority: AUTHORITY_ID_INPUT,
    assetId: CANONICAL_ASSET_ID_INPUT,
    quantity: "10",
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildTransferAssetTransaction returns canonical hash", () => {
  const built = buildTransferAssetTransaction({
    chainId: "test-chain",
    authority: AUTHORITY_ID_INPUT,
    sourceAssetId: CANONICAL_ASSET_ID_INPUT,
    quantity: "3",
    destinationAccountId: AUTHORITY_ID_INPUT,
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildTransferRwaTransaction returns canonical hash", () => {
  const built = buildTransferRwaTransaction({
    chainId: "test-chain",
    authority: AUTHORITY_ID_INPUT,
    sourceAccountId: AUTHORITY_ID_INPUT,
    rwaId: RWA_ID,
    quantity: "3",
    destinationAccountId: AUTHORITY_ID_INPUT,
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

baseTest("transaction builders reject padded authority and asset definition IDs before native dispatch", () => {
  const calls = [];
  withNativeBinding(
    {
      buildTransaction: () => {
        calls.push("buildTransaction");
        return {
          signed_transaction: Buffer.from([0x47]),
          hash: Buffer.alloc(32, 0x47),
        };
      },
    },
    () => {
      assert.throws(
        () =>
          buildTransaction({
            chainId: "test-chain",
            authority: ` ${AUTHORITY_ID_INPUT}`,
            instructions: [{ RegisterDomain: { id: "wonderland" } }],
            privateKey: PRIVATE_KEY,
          }),
        /authority must not contain surrounding whitespace/u,
      );
      assert.throws(
        () =>
          buildRegisterAssetDefinitionAndMintTransaction({
            chainId: "test-chain",
            authority: AUTHORITY_ID_INPUT,
            assetDefinition: {
              assetDefinitionId: `${ASSET_DEFINITION_ID} `,
              spec: { scale: 0 },
            },
            privateKey: PRIVATE_KEY,
          }),
        /assetDefinition\.assetDefinitionId must not contain surrounding whitespace/u,
      );
    },
  );
  assert.deepEqual(calls, []);
});

test("buildRegisterRwaTransaction forwards canonical instruction payload", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((json) => JSON.parse(json)),
        });
        return {
          signed_transaction: Buffer.from([0x44]),
          hash: Buffer.alloc(32, 0xdd),
        };
      },
    },
    () =>
      buildRegisterRwaTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        rwa: {
          domain: "commodities.sora",
          quantity: "10.5",
          spec: { scale: 1 },
          primaryReference: "vault-cert-001",
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  assert.equal(captures[0].authority, AUTHORITY_ID);
  assert.deepEqual(captures[0].instructions[0], {
    RegisterRwa: {
      rwa: {
        domain: "commodities.sora",
        quantity: "10.5",
        spec: { scale: 1 },
        primary_reference: "vault-cert-001",
        status: null,
        metadata: {},
        parents: [],
        controls: {
          controller_accounts: [],
          controller_roles: [],
          freeze_enabled: false,
          hold_enabled: false,
          force_transfer_enabled: false,
          redeem_enabled: false,
        },
      },
    },
  });
});

test("buildRwaKeyValueTransactions forward canonical instruction payloads", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((json) => JSON.parse(json)),
        });
        return {
          signed_transaction: Buffer.from([0x45]),
          hash: Buffer.alloc(32, 0xee),
        };
      },
    },
    () => {
      buildSetRwaKeyValueTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        rwaId: RWA_ID,
        key: "grade",
        value: { origin: "AE", score: BigInt(9) },
        privateKey: PRIVATE_KEY,
      });
      buildRemoveRwaKeyValueTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        rwaId: RWA_ID,
        key: "grade",
        privateKey: PRIVATE_KEY,
      });
    },
  );
  assert.equal(captures.length, 2);
  assert.equal(captures[0].authority, AUTHORITY_ID);
  assert.deepEqual(captures[0].instructions[0], {
    SetRwaKeyValue: {
      rwa: RWA_ID,
      key: "grade",
      value: { origin: "AE", score: "9" },
    },
  });
  assert.deepEqual(captures[1].instructions[0], {
    RemoveRwaKeyValue: {
      rwa: RWA_ID,
      key: "grade",
    },
  });
});

test("buildMintAndTransferTransaction composes instructions in order", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x10, 0x20]),
    hash: Buffer.alloc(32, 0xbb),
  };
  withNativeBinding(
    {
      buildTransaction: (_, __, instructions) => {
        captures.push(instructions.map((payload) => JSON.parse(payload)));
        return fakeResult;
      },
    },
    () => {
      const result = buildMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        mint: { assetId: ASSET_ID_INPUT, quantity: "6" },
        transfer: {
          quantity: "2",
          destinationAccountId: AUTHORITY_ID_INPUT,
        },
        privateKey: PRIVATE_KEY,
      });
      assert.deepEqual(result.hash, Buffer.from(fakeResult.hash));
    },
  );
  assert.equal(captures.length, 1);
  const [mintInstruction, transferInstruction] = captures[0];
  assert.deepEqual(mintInstruction, {
    Mint: { Asset: { destination: ASSET_ID, object: "6" } },
  });
  assert.deepEqual(transferInstruction, {
    Transfer: {
      Asset: {
        source: ASSET_ID,
        object: "2",
        destination: AUTHORITY_ID,
      },
    },
  });
});

test("buildRegisterDomainAndMintTransaction supports mint arrays", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((j) => JSON.parse(j)),
        });
        return {
          signed_transaction: Buffer.from([0x30]),
          hash: Buffer.alloc(32, 0xcc),
        };
      },
    },
    () =>
      buildRegisterDomainAndMintTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        domain: { domainId: "wonderland.sora" },
        mints: [
          { assetId: ASSET_ID_INPUT, quantity: "4" },
          { assetId: CANONICAL_LILY_ASSET_ID_INPUT, quantity: "1" },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 3);
  assert.deepEqual(
    instructions[0],
    buildRegisterDomainInstruction({ domainId: "wonderland.sora" }),
  );
  assert.deepEqual(instructions[1], {
    Mint: { Asset: { destination: ASSET_ID, object: "4" } },
  });
  assert.deepEqual(instructions[2], {
    Mint: {
      Asset: {
        destination: CANONICAL_LILY_ASSET_ID_INPUT,
        object: "1",
      },
    },
  });
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction supports transfer arrays", () => {
  const captures = [];
  const secondAccountIdPublicKeyHex =
    "1AA70BFDE38BFD7CBE6AD29E59F290D4A4B0DD02792C0CE7371477C4E0D62759";
  const secondAccountId = i105FromEd25519PublicKeyHex(
    secondAccountIdPublicKeyHex,
  );
  const secondAccountIdInput = i105FromEd25519PublicKeyHex(
    secondAccountIdPublicKeyHex,
  );
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((j) => JSON.parse(j)),
        });
        return {
          signed_transaction: Buffer.from([0x31]),
          hash: Buffer.alloc(32, 0xdd),
        };
      },
    },
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mints: [
          { assetId: CANONICAL_ASSET_ID_INPUT, quantity: "7" },
          { assetId: SECOND_CANONICAL_ASSET_ID_INPUT, quantity: "2" },
        ],
        transfers: [
          { quantity: "5", destinationAccountId: AUTHORITY_ID_INPUT },
          {
            sourceAssetId: SECOND_CANONICAL_ASSET_ID_INPUT,
            quantity: "1",
            destinationAccountId: secondAccountIdInput,
          },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 5);
  assert.deepEqual(instructions[1], {
    Mint: {
      Asset: {
        destination: CANONICAL_ASSET_ID_INPUT,
        object: "7",
      },
    },
  });
  assert.deepEqual(instructions[2], {
    Mint: {
      Asset: {
        destination: SECOND_CANONICAL_ASSET_ID_INPUT,
        object: "2",
      },
    },
  });
  assert.deepEqual(instructions[3], {
    Transfer: {
      Asset: {
        source: CANONICAL_ASSET_ID_INPUT,
        object: "5",
        destination: AUTHORITY_ID,
      },
    },
  });
  assert.deepEqual(instructions[4], {
    Transfer: {
      Asset: {
        source: SECOND_CANONICAL_ASSET_ID_INPUT,
        object: "1",
        destination: secondAccountId,
      },
    },
  });
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction derives asset ids from accountId", () => {
  const captures = [];
  withNativeBinding(
    {
      encodeAssetId: encodeAssetIdForKnownAccount,
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((j) => JSON.parse(j)),
        });
        return {
          signed_transaction: Buffer.from([0x32]),
          hash: Buffer.alloc(32, 0xee),
        };
      },
    },
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
        mints: [
          {
            accountId: AUTHORITY_ID_INPUT,
            assetId: CANONICAL_ASSET_ID_INPUT,
            quantity: "1",
          },
        ],
        transfers: [
          { quantity: "1", destinationAccountId: AUTHORITY_ID_INPUT },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  assert.deepEqual(captures[0].instructions[1], {
    Mint: {
      Asset: {
        destination: CANONICAL_ASSET_ID_INPUT,
        object: "1",
      },
    },
  });
});

test("buildMintAndTransferTransaction returns canonical hash", () => {
  const built = buildMintAndTransferTransaction({
    chainId: "test-chain",
    authority: AUTHORITY_ID_INPUT,
    mint: { assetId: CANONICAL_ASSET_ID_INPUT, quantity: "8" },
    transfer: {
      sourceAssetId: CANONICAL_ASSET_ID_INPUT,
      quantity: "3",
      destinationAccountId: AUTHORITY_ID_INPUT,
    },
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction returns canonical hash", () => {
  const built = buildRegisterAssetDefinitionMintAndTransferTransaction({
    chainId: "test-chain",
    authority: AUTHORITY_ID_INPUT,
    assetDefinition: { assetDefinitionId: ASSET_DEFINITION_ID },
    mint: { assetId: CANONICAL_ASSET_ID_INPUT, quantity: "4" },
    transfer: {
      sourceAssetId: CANONICAL_ASSET_ID_INPUT,
      destinationAccountId: AUTHORITY_ID_INPUT,
      quantity: "1",
    },
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildCreateKaigiTransaction composes Kaigi create instruction", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x40]),
    hash: Buffer.alloc(32, 0x55),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
        });
        return fakeResult;
      },
    },
    () =>
      buildCreateKaigiTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        call: {
          id: { domainId: "wonderland.sora", callName: "weekly-sync" },
          host: AUTHORITY_ID_INPUT,
          gasRatePerMinute: 120,
          relayManifest: {
            expiryMs: 1700111000000,
            hops: [
              {
                relayId: RELAY_ACCOUNT_ID_INPUT,
                hpkePublicKey: Buffer.alloc(32, 0x01),
                weight: 2,
              },
            ],
          },
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 1);
  const created = instructions[0];
  assert.deepEqual(created.Kaigi.CreateKaigi.call.id, {
    domain_id: "wonderland.sora",
    call_name: "weekly-sync",
  });
  assert.equal(created.Kaigi.CreateKaigi.call.gas_rate_per_minute, 120);
  assert.deepEqual(created.Kaigi.CreateKaigi.call.relay_manifest.hops[0], {
    relay_id: RELAY_ACCOUNT_ID,
    hpke_public_key: "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
    weight: 2,
  });
  assert.equal(created.Kaigi.CreateKaigi.commitment, null);
});

test("buildCreateKaigiTransaction preserves privacy artifacts", () => {
  const captures = [];
  const commitment = Buffer.alloc(32, 0x33);
  const nullifier = Buffer.alloc(32, 0x44);
  const proof = Buffer.from([0xfa, 0xce]);
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
        });
        return {
          signed_transaction: Buffer.from([0x40]),
          hash: Buffer.alloc(32, 0x55),
        };
      },
    },
    () =>
      buildCreateKaigiTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        call: {
          id: "wonderland.sora:private-room",
          host: AUTHORITY_ID_INPUT,
          privacyMode: "ZkRosterV1",
          commitment: { commitment, aliasTag: "host" },
          nullifier: { digest: nullifier, issuedAtMs: 12 },
          proof,
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  const [{ instructions }] = captures;
  const created = instructions[0].Kaigi.CreateKaigi;
  assert.equal(created.commitment.commitment, normalizedHashHex(commitment));
  assert.equal(created.nullifier.digest, normalizedHashHex(nullifier));
  assert.equal(created.proof, proof.toString("base64"));
});

test("buildJoinKaigiTransaction normalizes binary fields", () => {
  const commitment = Buffer.alloc(32, 0x77);
  const nullifier = Buffer.alloc(32, 0x22);
  const proof = Buffer.from([0xaa, 0xbb, 0xcc, 0xdd]);
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x41]),
    hash: Buffer.alloc(32, 0x66),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
        });
        return fakeResult;
      },
    },
    () =>
      buildJoinKaigiTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        join: {
          callId: "wonderland.sora:weekly-sync",
          participant: AUTHORITY_ID_INPUT,
          commitment: { commitment, aliasTag: "alice" },
          nullifier: { digest: nullifier, issuedAtMs: 42 },
          proof,
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  const joinInstruction = instructions[0].Kaigi.JoinKaigi;
  assert.equal(joinInstruction.participant, AUTHORITY_ID);
  assert.equal(
    joinInstruction.commitment.commitment,
    normalizedHashHex(commitment),
  );
  assert.equal(joinInstruction.nullifier.digest, normalizedHashHex(nullifier));
  assert.equal(joinInstruction.nullifier.issued_at_ms, 42);
  assert.equal(joinInstruction.proof, proof.toString("base64"));
});

test("buildRegisterKaigiRelayTransaction encodes hpke key", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x42]),
    hash: Buffer.alloc(32, 0x77),
  };
  const relayId = RELAY_ACCOUNT_ID;
  const relayIdInput = RELAY_ACCOUNT_ID_INPUT;
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
        });
        return fakeResult;
      },
    },
    () =>
      buildRegisterKaigiRelayTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        relay: {
          relayId: relayIdInput,
          hpkePublicKey: Buffer.alloc(32, 0xaa),
          bandwidthClass: 6,
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  const relayInstruction = instructions[0].Kaigi.RegisterKaigiRelay;
  assert.equal(relayInstruction.relay.relay_id, relayId);
  assert.equal(
    relayInstruction.relay.hpke_public_key,
    "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=",
  );
  assert.equal(relayInstruction.relay.bandwidth_class, 6);
});

test("buildProposeDeployContractTransaction wraps proposal", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x10]),
    hash: Buffer.alloc(32, 0x10),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
        });
        return fakeResult;
      },
    },
    () =>
      buildProposeDeployContractTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        proposal: {
          contractAddress:
            "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
          codeHash: "aa".repeat(32),
          abiHash: "bb".repeat(32),
          window: { lower: 1, upper: 2 },
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const propose = captures[0].instructions[0].ProposeDeployContract;
  assert.equal(
    propose.contract_address,
    "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
  );
});

test("buildCastZkBallotTransaction encodes ballot", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x11]),
    hash: Buffer.alloc(32, 0x11),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildCastZkBallotTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        ballot: {
          electionId: "ref-1",
          proof: Buffer.alloc(32, 0x01),
          publicInputs: { tally: "aye" },
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures[0].CastZkBallot.election_id, "ref-1");
});

test("buildCastPlainBallotTransaction normalizes amount", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x12]),
    hash: Buffer.alloc(32, 0x12),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildCastPlainBallotTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        ballot: {
          referendumId: "ref-2",
          owner: AUTHORITY_ID_INPUT,
          amount: 10,
          durationBlocks: 5,
          direction: "aye",
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures[0].CastPlainBallot.direction, 0);
});

test("buildEnactReferendumTransaction wraps enactment", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x13]),
    hash: Buffer.alloc(32, 0x13),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildEnactReferendumTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        enactment: {
          referendumId: Buffer.alloc(32, 0x33),
          preimageHash: Buffer.alloc(32, 0x44),
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.ok(captures[0].EnactReferendum);
});

test("buildFinalizeReferendumTransaction normalizes proposal id", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x14]),
    hash: Buffer.alloc(32, 0x14),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildFinalizeReferendumTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        finalization: {
          referendumId: "ref-3",
          proposalId: Buffer.alloc(32, 0x55),
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.deepEqual(
    captures[0].FinalizeReferendum.proposal_id,
    toByteArray(Buffer.alloc(32, 0x55)),
  );
});

test("buildPersistCouncilForEpochTransaction wraps council", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x15]),
    hash: Buffer.alloc(32, 0x15),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildPersistCouncilForEpochTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        record: {
          epoch: 1,
          members: [AUTHORITY_ID_INPUT],
          candidatesCount: 5,
          derivedBy: "Vrf",
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures[0].PersistCouncilForEpoch.members.length, 1);
});

test("buildRegisterSmartContractCodeTransaction wraps manifest instruction", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x01]),
    hash: Buffer.alloc(32, 0xbb),
  };
  withNativeBinding(
    {
      buildTransaction: (
        chainId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          chainId,
          authority,
          instructions,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      const result = buildRegisterSmartContractCodeTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        manifest: {
          codeHash: Buffer.alloc(32, 0xaa),
          compilerFingerprint: "rustc",
        },
        privateKey: PRIVATE_KEY,
      });
      assert.ok(Buffer.isBuffer(result.hash));
    },
  );
  assert.equal(captures.length, 1);
  const parsed = JSON.parse(captures[0].instructions[0]);
  assert.equal(
    parsed.RegisterSmartContractCode.manifest.compiler_fingerprint,
    "rustc",
  );
});

test("buildRegisterSmartContractBytesTransaction encodes code payload", () => {
  const codeBytes = Buffer.from([0xde, 0xad]);
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x02]),
    hash: Buffer.alloc(32, 0xcc),
  };
  withNativeBinding(
    {
      buildTransaction: (
        chainId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          chainId,
          authority,
          instructions,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      const result = buildRegisterSmartContractBytesTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        codeHash: Buffer.alloc(32, 0xdd),
        code: codeBytes,
        privateKey: PRIVATE_KEY,
      });
      assert.ok(Buffer.isBuffer(result.signedTransaction));
    },
  );
  const parsed = JSON.parse(captures[0].instructions[0]);
  assert.equal(
    parsed.RegisterSmartContractBytes.code,
    codeBytes.toString("base64"),
  );
});

test("buildRemoveSmartContractBytesTransaction wraps removal payload", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x04]),
    hash: Buffer.alloc(32, 0xee),
  };
  withNativeBinding(
    {
      buildTransaction: (...args) => {
        captures.push(args[2]);
        return fakeResult;
      },
    },
    () => {
      buildRemoveSmartContractBytesTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        codeHash: Buffer.alloc(32, 0xaa),
        reason: "cleanup",
        privateKey: PRIVATE_KEY,
      });
    },
  );
  const parsed = JSON.parse(captures[0][0]);
  assert.equal(parsed.RemoveSmartContractBytes.reason, "cleanup");
});

baseTest("proof builders reject padded inline verifier-key metadata", () => {
  const captures = [];
  const verifyingKey = {
    id: { backend: "halo2/ipa" },
    record: { circuit_id: "private-kaigi-fee-v1" },
    inlineKey: { bytesBase64: Buffer.from([1, 2, 3]).toString("base64") },
  };
  withNativeBinding(
    {
      buildPrivateKaigiFeeSpend: (
        chainId,
        assetDefinitionId,
        actionHash,
        anchorRootHex,
        feeAmount,
        backend,
        circuitId,
        bytes,
      ) => {
        captures.push({
          chainId,
          assetDefinitionId,
          actionHash,
          anchorRootHex,
          feeAmount,
          backend,
          circuitId,
          bytes: Buffer.from(bytes),
        });
        return {
          assetDefinitionId,
          anchorRoot: Buffer.alloc(32, 0x11),
          nullifiers: [],
          outputCommitments: [],
          encryptedChangePayloads: [],
          proof: Buffer.from("proof"),
        };
      },
    },
    () => {
      buildPrivateKaigiFeeSpend({
        chainId: "test-chain",
        assetDefinitionId: ASSET_DEFINITION_ID,
        actionHash: Buffer.alloc(32, 0xaa),
        anchorRootHex: Buffer.alloc(32, 0xbb).toString("hex"),
        feeAmount: "7",
        verifyingKey,
      });
    },
  );
  assert.equal(captures[0].backend, "halo2/ipa");
  assert.equal(captures[0].circuitId, "private-kaigi-fee-v1");
  assert.deepEqual(captures[0].bytes, Buffer.from([1, 2, 3]));

  for (const [label, patch, message] of [
    [
      "backend",
      { id: { backend: " halo2/ipa " } },
      /privateKaigiFeeSpend\.verifyingKey\.id\.backend must not contain surrounding whitespace/u,
    ],
    [
      "circuit",
      { record: { circuit_id: " private-kaigi-fee-v1 " } },
      /privateKaigiFeeSpend\.verifyingKey\.record\.circuit_id must not contain surrounding whitespace/u,
    ],
  ]) {
    assert.throws(
      () =>
        withNativeBinding(
          {
            buildPrivateKaigiFeeSpend: () => {
              throw new Error(
                `${label} metadata should fail before native call`,
              );
            },
          },
          () =>
            buildPrivateKaigiFeeSpend({
              chainId: "test-chain",
              assetDefinitionId: ASSET_DEFINITION_ID,
              actionHash: Buffer.alloc(32, 0xaa),
              anchorRootHex: Buffer.alloc(32, 0xbb).toString("hex"),
              feeAmount: "7",
              verifyingKey: { ...verifyingKey, ...patch },
            }),
        ),
      message,
    );
  }

  for (const [label, patch, message] of [
    [
      "chainId",
      { chainId: " test-chain" },
      /privateKaigiFeeSpend\.chainId must not contain surrounding whitespace/u,
    ],
    [
      "assetDefinitionId",
      { assetDefinitionId: `${ASSET_DEFINITION_ID} ` },
      /privateKaigiFeeSpend\.assetDefinitionId must not contain surrounding whitespace/u,
    ],
    [
      "anchorRootHex",
      { anchorRootHex: `${Buffer.alloc(32, 0xbb).toString("hex")}\n` },
      /privateKaigiFeeSpend\.anchorRootHex must not contain surrounding whitespace/u,
    ],
    [
      "feeAmount",
      { feeAmount: " 7" },
      /privateKaigiFeeSpend\.feeAmount must not contain surrounding whitespace/u,
    ],
  ]) {
    assert.throws(
      () =>
        withNativeBinding(
          {
            buildPrivateKaigiFeeSpend: () => {
              throw new Error(`${label} should fail before native call`);
            },
          },
          () =>
            buildPrivateKaigiFeeSpend({
              chainId: "test-chain",
              assetDefinitionId: ASSET_DEFINITION_ID,
              actionHash: Buffer.alloc(32, 0xaa),
              anchorRootHex: Buffer.alloc(32, 0xbb).toString("hex"),
              feeAmount: "7",
              verifyingKey,
              ...patch,
            }),
        ),
      message,
    );
  }
});

baseTest("confidential proof builders reject padded chain IDs and hex fields before native dispatch", () => {
  const calls = [];
  const verifyingKey = {
    id: { backend: "halo2/ipa" },
    record: { circuit_id: "confidential-transfer-v2" },
    inlineKey: { bytesBase64: Buffer.from([1, 2, 3]).toString("base64") },
  };
  const spendKey = Buffer.alloc(32, 0x42);
  const rho = Buffer.alloc(32, 0x51).toString("hex");
  const diversifier = Buffer.alloc(32, 0x52).toString("hex");
  const ownerTag = Buffer.alloc(32, 0x53).toString("hex");
  const rootHint = Buffer.alloc(32, 0x54).toString("hex");
  const treeCommitment = Buffer.alloc(32, 0x55).toString("hex");
  const baseRequest = {
    chainId: "test-chain",
    assetDefinitionId: ASSET_DEFINITION_ID,
    spendKey,
    treeCommitments: [treeCommitment],
    inputs: [{ amount: "7", rhoHex: rho, diversifierHex: diversifier, leafIndex: 0 }],
    outputs: [{ amount: "7", rhoHex: rho, ownerTagHex: ownerTag }],
    rootHintHex: rootHint,
    verifyingKey,
  };
  withNativeBinding(
    {
      buildConfidentialTransferProofV2: (...args) => {
        calls.push(args);
        return {
          nullifiers: [],
          outputCommitments: [],
          root: Buffer.alloc(32, 0x61),
          proof: Buffer.from([0x62]),
        };
      },
      buildConfidentialUnshieldProofV2: () => {
        throw new Error("unshield v2 publicAmount should fail before native call");
      },
      buildConfidentialUnshieldProofV3: () => {
        throw new Error("unshield v3 publicAmount should fail before native call");
      },
    },
    () => {
      buildConfidentialTransferProofV2(baseRequest);
      assert.equal(calls.length, 1);
      assert.equal(calls[0][0], "test-chain");
      assert.equal(calls[0][1], ASSET_DEFINITION_ID);
      assert.deepEqual(calls[0][3], [treeCommitment]);
      assert.equal(calls[0][4][0].rhoHex, rho);
      assert.equal(calls[0][4][0].diversifierHex, diversifier);
      assert.equal(calls[0][5][0].ownerTagHex, ownerTag);

      calls.length = 0;
      for (const [label, patch, message] of [
        [
          "chainId",
          { chainId: " test-chain" },
          /confidentialTransferProofV2\.chainId must not contain surrounding whitespace/u,
        ],
        [
          "assetDefinitionId",
          { assetDefinitionId: `${ASSET_DEFINITION_ID} ` },
          /confidentialTransferProofV2\.assetDefinitionId must not contain surrounding whitespace/u,
        ],
        [
          "input amount",
          { inputs: [{ amount: " 7", rhoHex: rho, diversifierHex: diversifier }] },
          /inputs\[0\]\.amount must not contain surrounding whitespace/u,
        ],
        [
          "inputs rho",
          { inputs: [{ amount: "7", rhoHex: `${rho} `, diversifierHex: diversifier }] },
          /inputs\[0\]\.rho must not contain surrounding whitespace/u,
        ],
        [
          "input diversifier",
          { inputs: [{ amount: "7", rhoHex: rho, diversifierHex: ` ${diversifier}` }] },
          /inputs\[0\]\.diversifier must not contain surrounding whitespace/u,
        ],
        [
          "missing input diversifier",
          { inputs: [{ amount: "7", rhoHex: rho }] },
          /inputs\[0\]\.diversifier is required/u,
        ],
        [
          "input diversifier snake alias",
          { inputs: [{ amount: "7", rhoHex: rho, diversifier_hex: diversifier }] },
          /inputs\[0\]\.diversifier must use canonical diversifierHex/u,
        ],
        [
          "input diversifier raw alias",
          { inputs: [{ amount: "7", rhoHex: rho, diversifier: Buffer.alloc(32, 0x52) }] },
          /inputs\[0\]\.diversifier must use canonical diversifierHex/u,
        ],
        [
          "output amount",
          { outputs: [{ amount: "7\n", rhoHex: rho, ownerTagHex: ownerTag }] },
          /outputs\[0\]\.amount must not contain surrounding whitespace/u,
        ],
        [
          "output ownerTag",
          { outputs: [{ amount: "7", rhoHex: rho, ownerTagHex: `${ownerTag}\n` }] },
          /outputs\[0\]\.ownerTag must not contain surrounding whitespace/u,
        ],
        [
          "treeCommitments",
          { treeCommitments: [` ${treeCommitment}`] },
          /treeCommitments\[0\] must not contain surrounding whitespace/u,
        ],
        [
          "rootHintHex",
          { rootHintHex: `${rootHint} ` },
          /rootHintHex must not contain surrounding whitespace/u,
        ],
      ]) {
        assert.throws(
          () => buildConfidentialTransferProofV2({ ...baseRequest, ...patch }),
          message,
          label,
        );
      }

      assert.throws(
        () =>
          buildConfidentialUnshieldProofV2({
            chainId: "test-chain",
            assetDefinitionId: ASSET_DEFINITION_ID,
            spendKey,
            treeCommitments: [treeCommitment],
            inputs: [{ amount: "7", rhoHex: rho, diversifierHex: diversifier }],
            publicAmount: " 7",
            rootHintHex: rootHint,
            verifyingKey,
          }),
        /publicAmount must not contain surrounding whitespace/u,
      );
      assert.throws(
        () =>
          buildConfidentialUnshieldProofV3({
            chainId: "test-chain",
            assetDefinitionId: ASSET_DEFINITION_ID,
            spendKey,
            treeCommitments: [treeCommitment],
            inputs: [{ amount: "7", rhoHex: rho, diversifierHex: diversifier }],
            outputs: [{ amount: "7", rhoHex: rho }],
            publicAmount: "7\n",
            rootHintHex: rootHint,
            verifyingKey,
          }),
        /publicAmount must not contain surrounding whitespace/u,
      );
    },
  );

  assert.deepEqual(calls, []);
});

baseTest("private Kaigi transaction builders reject padded identifiers before native dispatch", () => {
  const calls = [];
  const nativeResult = (tag) => ({
    transactionEntrypoint: Buffer.from(`${tag}-entrypoint`),
    hash: Buffer.alloc(32, tag.charCodeAt(0)),
    actionHash: Buffer.alloc(32, tag.charCodeAt(0) + 1),
  });
  const binding = {
    buildPrivateCreateKaigiTransaction: (chainId, call, artifacts, feeSpend) => {
      calls.push(["create", chainId, call, artifacts, feeSpend]);
      return nativeResult("c");
    },
    buildPrivateJoinKaigiTransaction: (chainId, callId, artifacts, feeSpend) => {
      calls.push(["join", chainId, callId, artifacts, feeSpend]);
      return nativeResult("j");
    },
    buildPrivateEndKaigiTransaction: (chainId, callId, endedAtMs, artifacts, feeSpend) => {
      calls.push(["end", chainId, callId, endedAtMs, artifacts, feeSpend]);
      return nativeResult("e");
    },
  };
  withNativeBinding(binding, () => {
    buildPrivateCreateKaigiTransaction({
      chainId: "test-chain",
      call: { callId: "call-1" },
      artifacts: { proof: "proof" },
      feeSpend: { amount: "7" },
    });
    buildPrivateJoinKaigiTransaction({
      chainId: "test-chain",
      callId: "call-1",
      artifacts: { proof: "proof" },
      feeSpend: { amount: "7" },
    });
    buildPrivateEndKaigiTransaction({
      chainId: "test-chain",
      callId: "call-1",
      endedAtMs: 42,
      artifacts: { proof: "proof" },
      feeSpend: { amount: "7" },
    });
  });
  assert.deepEqual(calls.map((entry) => entry.slice(0, 3)), [
    ["create", "test-chain", '{"callId":"call-1"}'],
    ["join", "test-chain", "call-1"],
    ["end", "test-chain", "call-1"],
  ]);

  for (const [label, build, message] of [
    [
      "create chainId",
      () =>
        buildPrivateCreateKaigiTransaction({
          chainId: " test-chain",
          call: { callId: "call-1" },
          artifacts: {},
          feeSpend: {},
        }),
      /privateCreateKaigi\.chainId must not contain surrounding whitespace/u,
    ],
    [
      "join chainId",
      () =>
        buildPrivateJoinKaigiTransaction({
          chainId: "test-chain\n",
          callId: "call-1",
          artifacts: {},
          feeSpend: {},
        }),
      /privateJoinKaigi\.chainId must not contain surrounding whitespace/u,
    ],
    [
      "join callId",
      () =>
        buildPrivateJoinKaigiTransaction({
          chainId: "test-chain",
          callId: " call-1",
          artifacts: {},
          feeSpend: {},
        }),
      /privateJoinKaigi\.callId must not contain surrounding whitespace/u,
    ],
    [
      "end chainId",
      () =>
        buildPrivateEndKaigiTransaction({
          chainId: " test-chain",
          callId: "call-1",
          artifacts: {},
          feeSpend: {},
        }),
      /privateEndKaigi\.chainId must not contain surrounding whitespace/u,
    ],
    [
      "end callId",
      () =>
        buildPrivateEndKaigiTransaction({
          chainId: "test-chain",
          callId: "call-1 ",
          artifacts: {},
          feeSpend: {},
        }),
      /privateEndKaigi\.callId must not contain surrounding whitespace/u,
    ],
  ]) {
    const before = calls.length;
    assert.throws(
      () =>
        withNativeBinding(
          {
            buildPrivateCreateKaigiTransaction: () => {
              throw new Error(`${label} should fail before native call`);
            },
            buildPrivateJoinKaigiTransaction: () => {
              throw new Error(`${label} should fail before native call`);
            },
            buildPrivateEndKaigiTransaction: () => {
              throw new Error(`${label} should fail before native call`);
            },
          },
          build,
        ),
      message,
      label,
    );
    assert.equal(calls.length, before, label);
  }
});

test("confidential transaction builders wrap expected instruction payloads", () => {
  const encryptedPayload = {
    version: 1,
    ephemeralPublicKey: Buffer.alloc(32, 0x01),
    nonce: Buffer.alloc(24, 0x02),
    ciphertext: Buffer.from("note"),
  };
  const proof = {
    backend: "halo2/ipa",
    proof: Buffer.from("proof"),
    verifyingKeyRef: "halo2/ipa:vk_transfer",
  };
  const register = captureInstructionObject(() =>
    buildRegisterZkAssetTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      registration: {
        assetDefinitionId: ASSET_DEFINITION_ID,
        mode: "Hybrid",
        transferVerifyingKey: "halo2/ipa:vk_transfer",
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(register.zk?.RegisterZkAsset);

  const policy = captureInstructionObject(() =>
    buildScheduleConfidentialPolicyTransitionTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      transition: {
        assetDefinitionId: ASSET_DEFINITION_ID,
        newMode: "TransparentOnly",
        effectiveHeight: 5,
        transitionId: Buffer.alloc(32, 0xaa),
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(policy.zk?.ScheduleConfidentialPolicyTransition);

  const cancel = captureInstructionObject(() =>
    buildCancelConfidentialPolicyTransitionTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      cancellation: {
        assetDefinitionId: ASSET_DEFINITION_ID,
        transitionId: Buffer.alloc(32, 0xbb),
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(cancel.zk?.CancelConfidentialPolicyTransition);

  const shield = captureInstructionObject(() =>
    buildShieldTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      shield: {
        assetDefinitionId: ASSET_DEFINITION_ID,
        fromAccountId: AUTHORITY_ID_INPUT,
        amount: "10",
        noteCommitment: Buffer.alloc(32, 0x03),
        encryptedPayload,
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(shield.zk?.Shield);

  const transfer = captureInstructionObject(() =>
    buildZkTransferTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      transfer: {
        assetDefinitionId: ASSET_DEFINITION_ID,
        inputs: [Buffer.alloc(32, 0x10)],
        outputs: [Buffer.alloc(32, 0x20)],
        proof,
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(transfer.zk?.ZkTransfer);

  const unshield = captureInstructionObject(() =>
    buildUnshieldTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      unshield: {
        assetDefinitionId: ASSET_DEFINITION_ID,
        destinationAccountId: AUTHORITY_ID_INPUT,
        publicAmount: 3,
        inputs: [Buffer.alloc(32, 0x30)],
        proof,
        rootHint: Buffer.alloc(32, 0x40),
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(unshield.zk?.Unshield);

  const election = captureInstructionObject(() =>
    buildCreateElectionTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      election: {
        electionId: "election-1",
        options: 2,
        eligibleRoot: Buffer.alloc(32, 0x05),
        startTs: 1,
        endTs: 2,
        ballotVerifyingKey: "halo2/ipa:vk_ballot",
        tallyVerifyingKey: { backend: "halo2/ipa", name: "vk_tally" },
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(election.zk?.CreateElection);

  const ballot = captureInstructionObject(() =>
    buildSubmitBallotTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      ballot: {
        electionId: "election-1",
        ciphertext: Buffer.from("encrypted"),
        ballotProof: proof,
        nullifier: Buffer.alloc(32, 0x06),
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(ballot.zk?.SubmitBallot);

  const finalize = captureInstructionObject(() =>
    buildFinalizeElectionTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      finalization: {
        electionId: "election-1",
        tally: [1n],
        tallyProof: proof,
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(finalize.zk?.FinalizeElection);
});

function captureInstructionObject(buildFn) {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0xff]),
    hash: Buffer.alloc(32, 0xff),
  };
  withNativeBinding(
    {
      buildTransaction: (
        chainId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          chainId,
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    buildFn,
  );
  return captures[0].instructions[0];
}

function withNativeBinding(binding, fn) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = binding;
  try {
    return fn();
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }
}
