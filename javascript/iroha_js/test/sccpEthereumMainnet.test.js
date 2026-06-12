import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  EthereumMainnetBeaconRestConsensusProvider,
  EthereumMainnetSccp,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_ETH,
  SCCP_DOMAIN_SORA,
  SCCP_CODEC_EVM_HEX,
  SCCP_CODEC_TEXT_UTF8,
  SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
  SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
  SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
  SCCP_ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
  SCCP_ETH_MAINNET_EVM_CHAIN_ID,
  SCCP_ETH_MAINNET_NETWORK_ID,
  SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
  SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1,
  SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
  SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1,
  SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1,
  SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1,
  SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
  SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES,
  SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1,
  SCCP_TAIRA_XOR_ASSET_KEY_V1,
  buildEvmReceiptTrieProofFromReceipts,
  buildEthereumMainnetSccpLocalAdmissionSubmission,
  canonicalSccpMessageProofBundleBytes,
  canonicalSccpPayloadEnvelopeBytes,
  canonicalEvmReceiptRlp,
  canonicalEvmSccpReceiptProofBytes,
  ethereumMainnetSccpDestinationBinding,
  evmReceiptTrieKey,
  evmSccpReceiptProofHash,
  evmSccpSourceEventTopic,
  parseEthereumMainnetNativeEvmProverBundleManifest,
  parseEthereumMainnetNativeEvmProverParityFixture,
  parseEthereumMainnetNativeEvmProverSelfTestFixture,
  runEthereumMainnetNativeProverSelfTest,
  validateEthereumMainnetNativeEvmProverBundle,
  validateEthereumMainnetNativeEvmProverParityFixture,
  validateEthereumMainnetNativeEvmProverSelfTestFixture,
  verifyEthereumMainnetNativeEvmProverArtifacts,
  verifyEthereumMainnetNativeEvmProverArtifactsFromBundle,
  sccpMerkleRootFromCommitment,
  sccpPayloadHash,
  sccpTransferMessageId,
  wrapEvmSccpProofResult,
} from "../src/sccp.js";

const hex32 = (byte) => `0x${byte.repeat(32)}`;
const sha256Hex = (bytes) =>
  `0x${createHash("sha256").update(Buffer.from(bytes)).digest("hex")}`;
const fixtureHash = (label) => sha256Hex(Buffer.from(label, "utf8"));
const indexedHexBytes = (fillByte, length, index) => {
  const bytes = new Uint8Array(length).fill(fillByte);
  bytes[length - 2] = (index >>> 8) & 0xff;
  bytes[length - 1] = index & 0xff;
  return `0x${Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
};
const TX_HASH = hex32("aa");
const BLOCK_HASH = hex32("bb");
const SOURCE_EVENT_DIGEST = hex32("34");
const SOURCE_BRIDGE_ADDRESS = `0x${"44".repeat(20)}`;
const LOW_SYNC_COMMITTEE_BITS = `0x01${"00".repeat(63)}`;
const SAMPLE_SYNC_COMMITTEE_BITS = `0x${"ff".repeat(42)}3f${"00".repeat(21)}`;
const SAMPLE_SYNC_COMMITTEE_PARTICIPATION = "342";
const BEACON_HEADER_ROOT_SLOT_64 = "0xbb44a971e8c280f585ba430bfabfe87d9c59adf38bf9f77266b69687a148048c";
const BEACON_HEADER_ROOT_SLOT_96 = "0x503f2cd5b3926e0006f8ff49419e63d9588e13b792ef085b3639258112fa7ec2";
const SAMPLE_SYNC_COMMITTEE_SIGNATURE = `0x${"34".repeat(96)}`;
const SAMPLE_FINALITY_BRANCH = Array.from({ length: 6 }, (_, index) =>
  hex32((0x50 + index).toString(16).padStart(2, "0")),
);
const sampleFinalityUpdateFields = () => ({
  finalityBranch: SAMPLE_FINALITY_BRANCH,
  syncCommitteeBits: SAMPLE_SYNC_COMMITTEE_BITS,
  syncCommitteeSignature: SAMPLE_SYNC_COMMITTEE_SIGNATURE,
  syncCommitteeParticipation: SAMPLE_SYNC_COMMITTEE_PARTICIPATION,
  syncSignatureSlot: "65",
});
const sampleReceiptProof = {
  sourceDomain: SCCP_DOMAIN_ETH,
  sourceEventDigest: SOURCE_EVENT_DIGEST,
  beaconSlot: "64",
  finalityBranch: SAMPLE_FINALITY_BRANCH,
  executionBlockNumber: "4660",
  executionBlockHash: BLOCK_HASH,
  executionReceiptsRoot: hex32("cc"),
  beaconFinalizedRoot: hex32("dd"),
  syncCommitteeRoot: hex32("ee"),
  receiptRootIndex: "0",
  receiptTrieProofNodes: [[0xe4, 0x82, 0x20, 0x80, ...new Array(32).fill(0xbb)]],
  inclusionBranch: [hex32("f1")],
};

const sourceEventLog = (overrides = {}) => ({
  address: SOURCE_BRIDGE_ADDRESS,
  transactionHash: TX_HASH,
  blockHash: BLOCK_HASH,
  blockNumber: "0x1234",
  topics: [evmSccpSourceEventTopic(), SOURCE_EVENT_DIGEST],
  data: "0x",
  ...overrides,
});

const fullReceipt = (index, overrides = {}) => ({
  type: "0x2",
  transactionHash: index === 0 ? TX_HASH : hex32("ac"),
  transactionIndex: `0x${index.toString(16)}`,
  blockHash: BLOCK_HASH,
  blockNumber: "0x1234",
  status: "0x1",
  cumulativeGasUsed: `0x${(21_000 * (index + 1)).toString(16)}`,
  logsBloom: `0x${"00".repeat(256)}`,
  logs: index === 0 ? [sourceEventLog()] : [],
  ...overrides,
});

const sampleBlockReceipts = () => [fullReceipt(0), fullReceipt(1)];

const buildSampleOutboundBundleFixture = ({
  targetDomain = SCCP_DOMAIN_ETH,
  nonce = 1n,
} = {}) => {
  const transferPayload = {
    version: 1,
    source_domain: SCCP_DOMAIN_SORA,
    dest_domain: targetDomain,
    nonce,
    asset_home_domain: SCCP_DOMAIN_SORA,
    asset_id_codec: SCCP_CODEC_TEXT_UTF8,
    asset_id: SCCP_TAIRA_XOR_ASSET_KEY_V1,
    amount: 1000n,
    sender_codec: SCCP_CODEC_TEXT_UTF8,
    sender: "alice@sora",
    recipient_codec: SCCP_CODEC_EVM_HEX,
    recipient: `0x${"11".repeat(20)}`,
    route_id_codec: SCCP_CODEC_TEXT_UTF8,
    route_id:
      targetDomain === SCCP_DOMAIN_BSC
        ? SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1
        : "sccp-eth-mainnet-xor-route-v1",
  };
  const payloadEnvelope = { kind: "Transfer", value: transferPayload };
  const payloadBytes = canonicalSccpPayloadEnvelopeBytes(payloadEnvelope);
  const messageId = sccpTransferMessageId(transferPayload);
  const payloadHash = sccpPayloadHash(payloadBytes);
  const commitment = {
    version: 1,
    kind: "Transfer",
    target_domain: targetDomain,
    message_id: messageId,
    payload_hash: payloadHash,
  };
  const commitmentRoot = sccpMerkleRootFromCommitment(commitment, {
    steps: [],
  });
  const bundle = {
    version: 1,
    commitment_root: commitmentRoot,
    commitment,
    merkle_proof: { steps: [] },
    payload: payloadEnvelope,
    finality_proof: "0x010203",
  };
  return {
    publicInputs: {
      messageId,
      payloadHash,
      targetDomain,
      commitmentRoot,
      finalityHeight: "42",
      finalityBlockHash: hex32("55"),
    },
    bundleBytes: canonicalSccpMessageProofBundleBytes(bundle),
  };
};
const sampleOutboundFixture = buildSampleOutboundBundleFixture();
const samplePublicInputs = sampleOutboundFixture.publicInputs;

const sampleDestinationBindingInput = (overrides = {}) => ({
  verifierAddress: `0x${"11".repeat(20)}`,
  bridgeAddress: `0x${"22".repeat(20)}`,
  verifierCodeHash: hex32("bb"),
  verifierKeyHash: hex32("cc"),
  ...overrides,
});

const sampleOutboundInput = (
  targetDomain = SCCP_DOMAIN_ETH,
  destinationBindingOverrides = {},
) => {
  const fixture =
    targetDomain === SCCP_DOMAIN_ETH
      ? sampleOutboundFixture
      : buildSampleOutboundBundleFixture({ targetDomain });
  return {
    publicInputs: { ...fixture.publicInputs },
    bundleBytes: fixture.bundleBytes,
    destinationBinding: ethereumMainnetSccpDestinationBinding(
      sampleDestinationBindingInput(destinationBindingOverrides),
    ),
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: hex32("66"),
  };
};

const sampleNativeEvmProverBundle = (destinationBindingHash, overrides = {}) => {
  const proofArtifactHash = hex32("91");
  const provingKeyHash = hex32("92");
  return {
    schema: SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
    bundle_id: SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
    domain: SCCP_DOMAIN_ETH,
    chain: "eth",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact: "artifacts/eth-mainnet/proof-artifact.r1cs",
    proof_artifact_hash: proofArtifactHash,
    proving_key: "artifacts/eth-mainnet/proving-key.zkey",
    proving_key_hash: provingKeyHash,
    verifier_key: "artifacts/eth-mainnet/verifier-key.bin",
    verifier_key_hash: hex32("cc"),
    destination_binding_hash: destinationBindingHash,
    no_wasm: true,
    remote_prover_required: false,
    browser_implementation: "pure-typescript",
    cross_sdk_fixture_parity_artifact: "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
    native_prover_self_test_artifact: "artifacts/eth-mainnet/native-prover-self-test.json",
    native_sdk_artifacts: Object.entries(
      SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
    ).map(([sdk, implementation], index) => ({
      sdk,
      implementation,
      prover_artifact_hash: proofArtifactHash,
      proving_key_hash: provingKeyHash,
      implementation_artifact: `artifacts/eth-mainnet/${sdk}-implementation.bin`,
      implementation_hash: hex32((index + 1).toString(16).padStart(2, "0")),
    })),
    audit_hashes: {
      circuit_security_audit: fixtureHash("eth circuit security audit"),
      native_implementation_audit: fixtureHash(
        "eth native implementation audit",
      ),
      reproducible_build_attestation: fixtureHash(
        "eth reproducible build attestation",
      ),
      cross_sdk_fixture_parity: fixtureHash("eth cross-SDK fixture parity"),
      native_prover_self_test: fixtureHash("eth native prover self-test"),
      no_wasm_no_remote_scan: fixtureHash("eth no-wasm no-remote scan"),
    },
    ...overrides,
  };
};

const sampleNativeEvmProverParityFixture = (bundle, overrides = {}) => {
  const publicSignalWords = Array.from({ length: 9 }, (_, index) =>
    hex32((index + 0x10).toString(16).padStart(2, "0")),
  );
  const destinationBindingHash = bundle.destination_binding_hash ?? bundle.destinationBindingHash;
  const result = {
    receipt_proof_hash: hex32("d1"),
    source_proof_hash: hex32("d2"),
    destination_binding_hash: destinationBindingHash,
    public_signal_words: publicSignalWords,
    calldata_hash: hex32("d3"),
    torii_submit_payload_hash: hex32("d4"),
  };
  return {
    schema: SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
    domain: SCCP_DOMAIN_ETH,
    chain: "eth",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact_hash: bundle.proof_artifact_hash ?? bundle.proofArtifactHash,
    proving_key_hash: bundle.proving_key_hash ?? bundle.provingKeyHash,
    verifier_key_hash: bundle.verifier_key_hash ?? bundle.verifierKeyHash,
    destination_binding_hash: destinationBindingHash,
    production_attestation_hash: fixtureHash(
      "eth native prover parity production attestation",
    ),
    ...result,
    sdk_results: Object.fromEntries(
      Object.keys(SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1).map((sdk) => [
        sdk,
        { ...result },
      ]),
    ),
    ...overrides,
  };
};

const sampleNativeEvmProverParityFixtureBytes = (bundle, overrides = {}) =>
  Buffer.from(JSON.stringify(sampleNativeEvmProverParityFixture(bundle, overrides)), "utf8");

const sampleNativeEvmProverSelfTestFixture = (bundle, overrides = {}) => {
  const publicSignalWords = Array.from({ length: 9 }, (_, index) =>
    hex32((index + 0x30).toString(16).padStart(2, "0")),
  );
  const destinationBindingHash = bundle.destination_binding_hash ?? bundle.destinationBindingHash;
  const result = {
    request_hash: hex32("e1"),
    witness_hash: hex32("e2"),
    source_proof_hash: hex32("e3"),
    proof_hash: hex32("e4"),
    public_signal_words: publicSignalWords,
    calldata_hash: hex32("e5"),
    torii_submit_payload_hash: hex32("e6"),
  };
  return {
    schema: SCCP_ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
    domain: SCCP_DOMAIN_ETH,
    chain: "eth",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact_hash: bundle.proof_artifact_hash ?? bundle.proofArtifactHash,
    proving_key_hash: bundle.proving_key_hash ?? bundle.provingKeyHash,
    verifier_key_hash: bundle.verifier_key_hash ?? bundle.verifierKeyHash,
    destination_binding_hash: destinationBindingHash,
    production_attestation_hash: fixtureHash(
      "eth native prover self-test production attestation",
    ),
    ...result,
    sdk_results: Object.fromEntries(
      Object.keys(SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1).map((sdk) => [
        sdk,
        { ...result },
      ]),
    ),
    ...overrides,
  };
};

const sampleNativeEvmProverSelfTestFixtureBytes = (bundle, overrides = {}) =>
  Buffer.from(JSON.stringify(sampleNativeEvmProverSelfTestFixture(bundle, overrides)), "utf8");

const sampleNativeEvmProverBundleWithFixtureBytes = (
  destinationBindingHash,
  overrides = {},
) => {
  const draftBundle = sampleNativeEvmProverBundle(destinationBindingHash, overrides);
  const parityFixtureBytes = sampleNativeEvmProverParityFixtureBytes(draftBundle);
  const parityFixtureHash = sha256Hex(parityFixtureBytes);
  const selfTestFixtureBytes = sampleNativeEvmProverSelfTestFixtureBytes(draftBundle);
  const selfTestFixtureHash = sha256Hex(selfTestFixtureBytes);
  const bundle = sampleNativeEvmProverBundle(destinationBindingHash, {
    ...overrides,
    audit_hashes: {
      ...draftBundle.audit_hashes,
      cross_sdk_fixture_parity: parityFixtureHash,
      native_prover_self_test: selfTestFixtureHash,
    },
  });
  return {
    bundle,
    parityFixtureBytes: sampleNativeEvmProverParityFixtureBytes(bundle),
    parityFixtureHash,
    selfTestFixtureBytes: sampleNativeEvmProverSelfTestFixtureBytes(bundle),
    selfTestFixtureHash,
  };
};

const nativeEvmProverArtifactBytes = (label, size = 96 * 1024) => {
  const seed = Buffer.from(`${label}\n`, "utf8");
  const out = Buffer.alloc(size);
  for (let index = 0; index < out.length; index += 1) {
    out[index] = (seed[index % seed.length] + index * 31 + (index >> 7)) & 0xff;
  }
  return out;
};

const nativeEvmSnarkjsArtifactBytes = (
  label,
  magic,
  sectionCount,
  size = 96 * 1024,
) => {
  const out = nativeEvmProverArtifactBytes(label, size);
  const headerBytes = 12;
  const sectionHeaderBytes = sectionCount * 12;
  const payloadBytes = out.length - headerBytes - sectionHeaderBytes;
  if (payloadBytes < sectionCount) {
    throw new Error("native EVM SnarkJS fixture is too small");
  }
  out.set(Buffer.from(magic, "ascii"), 0);
  out.writeUInt32LE(1, 4);
  out.writeUInt32LE(sectionCount, 8);
  let offset = headerBytes;
  for (let index = 0; index < sectionCount; index += 1) {
    const sectionSize =
      Math.floor(payloadBytes / sectionCount) +
      (index < payloadBytes % sectionCount ? 1 : 0);
    out.writeUInt32LE(index + 1, offset);
    out.writeUInt32LE(sectionSize, offset + 4);
    out.writeUInt32LE(0, offset + 8);
    offset += 12 + sectionSize;
  }
  if (offset !== out.length) {
    throw new Error("native EVM SnarkJS fixture sections do not fill the file");
  }
  return out;
};

const sampleVerifiedNativeEvmProverFixture = () => {
  const proofArtifactBytes = nativeEvmSnarkjsArtifactBytes(
    "sccp proof artifact v1",
    "r1cs",
    3,
  );
  const provingKeyBytes = nativeEvmSnarkjsArtifactBytes(
    "sccp proving key v1",
    "zkey",
    10,
  );
  const verifierKeyBytes = nativeEvmProverArtifactBytes("sccp verifier key v1");
  const implementationBytes = nativeEvmProverArtifactBytes(
    "sccp pure typescript prover artifact v1",
  );
  const proofArtifactHash = sha256Hex(proofArtifactBytes);
  const provingKeyHash = sha256Hex(provingKeyBytes);
  const verifierKeyHash = sha256Hex(verifierKeyBytes);
  const implementationHash = sha256Hex(implementationBytes);
  const destinationBinding = ethereumMainnetSccpDestinationBinding(
    sampleDestinationBindingInput({ verifierKeyHash }),
  );
  const { bundle, parityFixtureBytes, parityFixtureHash, selfTestFixtureBytes, selfTestFixtureHash } =
    sampleNativeEvmProverBundleWithFixtureBytes(destinationBinding.bindingHash, {
    proof_artifact_hash: proofArtifactHash,
    proving_key_hash: provingKeyHash,
    verifier_key_hash: verifierKeyHash,
    native_sdk_artifacts: Object.entries(
      SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
    ).map(([sdk, implementation], index) => ({
      sdk,
      implementation,
      prover_artifact_hash: proofArtifactHash,
      proving_key_hash: provingKeyHash,
      implementation_artifact: `artifacts/eth-mainnet/${sdk}-implementation.bin`,
      implementation_hash: sdk === "javascript"
        ? implementationHash
        : hex32((index + 1).toString(16).padStart(2, "0")),
    })),
  });
  return {
    destinationBinding,
    nativeProverArtifacts: verifyEthereumMainnetNativeEvmProverArtifacts(
      {
        nativeProverBundle: bundle,
        proofArtifactBytes,
        provingKeyBytes,
        verifierKeyBytes,
        crossSdkFixtureParityBytes: parityFixtureBytes,
        nativeProverSelfTestBytes: selfTestFixtureBytes,
        sdk: "javascript",
        implementationBytes,
      },
      { destinationBinding },
    ),
    parityFixtureHash,
    selfTestFixtureHash,
  };
};

const abiWord = (value) => {
  let remaining = BigInt(value);
  const out = new Uint8Array(32);
  for (let index = out.length - 1; index >= 0; index -= 1) {
    out[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return out;
};

const BN254_G2_GENERATOR_WORDS = [
  abiWord(0x1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6edn),
  abiWord(0x198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2n),
  abiWord(0x12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daan),
  abiWord(0x090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975bn),
];

const groth16ProofBytes = (publicInputs = samplePublicInputs) => {
  const out = new Uint8Array(SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1);
  const words = [
    abiWord(1),
    Uint8Array.from(Buffer.from(publicInputs.messageId.slice(2), "hex")),
    abiWord(SCCP_DOMAIN_SORA),
    Uint8Array.from(Buffer.from(publicInputs.commitmentRoot.slice(2), "hex")),
    abiWord(1),
    abiWord(2),
    ...BN254_G2_GENERATOR_WORDS,
    abiWord(1),
    abiWord(2),
  ];
  words.forEach((word, index) => out.set(word, index * 32));
  return out;
};

const GROTH16_PROOF_BYTES = groth16ProofBytes();

const groth16ProofBytesWithWord = (
  index,
  word,
  publicInputs = samplePublicInputs,
) => {
  const out = groth16ProofBytes(publicInputs);
  out.set(word, index * 32);
  return out;
};

test("EthereumMainnetSccp validates EIP-1193 execution providers as Ethereum mainnet", async () => {
  const provider = {
    async request({ method }) {
      assert.equal(method, "eth_chainId");
      return "0x1";
    },
  };
  const sdk = new EthereumMainnetSccp({ executionProvider: provider });

  assert.equal(await sdk.validateExecutionProviderMainnet(), "0x1");
  assert.equal(SCCP_ETH_MAINNET_EVM_CHAIN_ID, 1);
});

test("EthereumMainnetSccp rejects non-mainnet execution providers", async () => {
  const sdk = new EthereumMainnetSccp({
    executionProvider: {
      async request() {
        return "0x38";
      },
    },
  });

  await assert.rejects(
    () => sdk.validateExecutionProviderMainnet(),
    /eth_chainId == 0x1/u,
  );
});

test("EthereumMainnetSccp rejects noncanonical JSON-RPC chain ids", async () => {
  for (const chainId of ["1", 1, "0x01", "0X1", " 0x1", "0x1 "]) {
    const sdk = new EthereumMainnetSccp({
      executionProvider: {
        async request() {
          return chainId;
        },
      },
    });

    await assert.rejects(
      () => sdk.validateExecutionProviderMainnet(),
      /canonical JSON-RPC quantity/u,
    );
  }
});

test("EthereumMainnetSccp collects receipt evidence from user execution and consensus providers", async () => {
  const calls = [];
  const provider = {
    async request({ method, params }) {
      calls.push([method, params]);
      if (method === "eth_chainId") return "0x1";
      if (method === "eth_getTransactionReceipt") {
        return {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
        };
      }
      if (method === "eth_getBlockByHash") {
        assert.deepEqual(params, [BLOCK_HASH, false]);
        return { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") };
      }
      throw new Error(`unexpected RPC method ${method}`);
    },
  };
  const consensusProvider = {
    async collectFinalityEvidence({ receipt, block, transactionHash }) {
      assert.equal(transactionHash, TX_HASH);
      assert.equal(receipt.blockHash, BLOCK_HASH);
      assert.equal(block.hash, BLOCK_HASH);
      return {
        finalizedHeaderRoot: hex32("dd"),
        syncCommitteeRoot: hex32("ee"),
        beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
        executionBlockNumber: "0x1234",
        executionBlockHash: BLOCK_HASH,
        executionReceiptsRoot: hex32("cc"),
      };
    },
  };
  const sdk = new EthereumMainnetSccp({
    executionProvider: provider,
    consensusProvider,
  });

  const evidence = await sdk.collectInboundEvidenceFromReceipt({
    transactionHash: TX_HASH,
  });

  assert.equal(evidence.sourceDomain, SCCP_DOMAIN_ETH);
  assert.equal(evidence.targetDomain, SCCP_DOMAIN_SORA);
  assert.equal(evidence.receipt.blockHash, BLOCK_HASH);
  assert.equal(evidence.block.hash, BLOCK_HASH);
  assert.equal(evidence.beaconFinality.finalizedHeaderRoot, hex32("dd"));
  assert.equal(evidence.beaconFinality.syncCommitteeRoot, hex32("ee"));
  assert.equal(evidence.beaconFinality.beaconSlot, "64");
  assert.equal(evidence.beaconFinality.executionBlockNumber, "4660");
  assert.equal(evidence.beaconFinality.executionBlockHash, BLOCK_HASH);
  assert.equal(evidence.beaconFinality.executionReceiptsRoot, hex32("cc"));
  assert.deepEqual(
    calls.map(([method]) => method),
    ["eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"],
  );
});

test("EthereumMainnetSccp collectInboundEvidenceFromReceipt snapshots consensus evidence", async () => {
  const mutableTopics = [evmSccpSourceEventTopic(), SOURCE_EVENT_DIGEST];
  const receiptLogs = [sourceEventLog({ topics: mutableTopics })];
  const receipt = fullReceipt(0, { logs: receiptLogs });
  const blockWitness = { branch: [hex32("e1")], bytes: new Uint8Array([0xbb]) };
  const block = {
    hash: BLOCK_HASH,
    number: "0x1234",
    receiptsRoot: hex32("cc"),
    mutableWitness: blockWitness,
  };
  const finalityBranch = [...SAMPLE_FINALITY_BRANCH];
  const finalityWitness = {
    branch: finalityBranch,
    bytes: new Uint8Array([0xcc]),
  };
  const mutablePayload = new Uint8Array([0xaa]);
  const sdk = new EthereumMainnetSccp({
    sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
    consensusProvider: {
      async collectFinalityEvidence(evidence) {
        assert.equal(Object.isFrozen(evidence), true);
        assert.equal(Object.isFrozen(evidence.receipt), true);
        assert.equal(Object.isFrozen(evidence.receipt.logs), true);
        assert.equal(Object.isFrozen(evidence.receipt.logs[0].topics), true);
        assert.equal(Object.isFrozen(evidence.block), true);
        assert.equal(Object.isFrozen(evidence.block.mutableWitness.branch), true);
        assert.equal(evidence.receipt.logs[0].topics[1], SOURCE_EVENT_DIGEST);
        assert.deepEqual([...evidence.block.mutableWitness.bytes], [0xbb]);
        assert.throws(() => {
          evidence.receipt.logs.push(sourceEventLog());
        }, TypeError);
        assert.throws(() => {
          evidence.block.mutableWitness.branch.push(hex32("99"));
        }, TypeError);

        receiptLogs.push(sourceEventLog());
        mutableTopics[1] = hex32("99");
        blockWitness.branch.push(hex32("99"));
        blockWitness.bytes[0] = 0x7c;
        return {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
          mutableWitness: finalityWitness,
        };
      },
    },
  });

  const evidence = await sdk.collectInboundEvidenceFromReceipt({
    receipt,
    block,
    mutablePayload,
  });
  finalityBranch[0] = hex32("99");
  finalityWitness.branch.push(hex32("99"));
  finalityWitness.bytes[0] = 0x7d;
  mutablePayload[0] = 0x7e;

  assert.equal(Object.isFrozen(evidence), true);
  assert.equal(Object.isFrozen(evidence.receipt.logs), true);
  assert.equal(Object.isFrozen(evidence.beaconFinality.finalityBranch), true);
  assert.equal(Object.isFrozen(evidence.beaconFinality.mutableWitness.branch), true);
  assert.throws(() => {
    evidence.receipt.logs.push(sourceEventLog());
  }, TypeError);
  assert.equal(evidence.mutablePayload[0], 0xaa);
  assert.equal(evidence.receipt.logs.length, 1);
  assert.equal(evidence.receipt.logs[0].topics[1], SOURCE_EVENT_DIGEST);
  assert.deepEqual(evidence.block.mutableWitness.branch, [hex32("e1")]);
  assert.deepEqual([...evidence.block.mutableWitness.bytes], [0xbb]);
  assert.deepEqual(evidence.beaconFinality.finalityBranch, SAMPLE_FINALITY_BRANCH);
  assert.deepEqual(evidence.beaconFinality.mutableWitness.branch, SAMPLE_FINALITY_BRANCH);
  assert.deepEqual([...evidence.beaconFinality.mutableWitness.bytes], [0xcc]);
});

test("Ethereum receipt trie helper uses RLP transaction-index keys", () => {
  assert.equal(evmReceiptTrieKey(0), "0x80");
  assert.equal(evmReceiptTrieKey(1), "0x01");
  assert.equal(evmReceiptTrieKey(128), "0x8180");
  assert.throws(() => evmReceiptTrieKey("0x01"), /unsigned integer|canonical JSON-RPC quantity/u);
  const receipt = fullReceipt(0);
  const receiptTrieProof = buildEvmReceiptTrieProofFromReceipts([receipt], {
    transactionIndex: 0,
  });
  assert.equal(canonicalEvmReceiptRlp(receipt)[0], 0x02);
  assert.equal(canonicalEvmReceiptRlp(fullReceipt(0, { type: "0x4" }))[0], 0x04);
  assert.equal(receiptTrieProof.receiptTrieKey, "0x80");
  assert.equal(
    receiptTrieProof.receiptRlp,
    `0x${Buffer.from(canonicalEvmReceiptRlp(receipt)).toString("hex")}`,
  );
  const zeroTopicReceiptTrieProof = buildEvmReceiptTrieProofFromReceipts(
    [
      receipt,
      fullReceipt(1, {
        logs: [
          {
            address: `0x${"12".repeat(20)}`,
            topics: [hex32("00")],
            data: "0x",
          },
        ],
      }),
    ],
    { transactionIndex: 0 },
  );
  assert.equal(
    zeroTopicReceiptTrieProof.receiptRlp,
    `0x${Buffer.from(canonicalEvmReceiptRlp(receipt)).toString("hex")}`,
  );
  const zeroAddressReceiptTrieProof = buildEvmReceiptTrieProofFromReceipts(
    [
      receipt,
      fullReceipt(1, {
        logs: [
          {
            address: `0x${"00".repeat(20)}`,
            topics: [hex32("44")],
            data: "0x",
          },
        ],
      }),
    ],
    { transactionIndex: 0 },
  );
  assert.equal(
    zeroAddressReceiptTrieProof.receiptRlp,
    `0x${Buffer.from(canonicalEvmReceiptRlp(receipt)).toString("hex")}`,
  );
  assert.match(receiptTrieProof.receiptsRoot, /^0x[0-9a-f]{64}$/u);
  assert.ok(receiptTrieProof.receiptTrieProofNodes.length > 0);
  assert.throws(
    () =>
      buildEvmReceiptTrieProofFromReceipts([fullReceipt(0, { transactionIndex: "0x1" })], {
        transactionIndex: 0,
      }),
    /transactionIndex/u,
  );
  assert.throws(
    () =>
      buildEvmReceiptTrieProofFromReceipts([fullReceipt(0, { transaction_index: "0x0" })], {
        transactionIndex: 0,
      }),
    /blockReceipts\[0\]\.transactionIndex must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildEvmReceiptTrieProofFromReceipts([fullReceipt(0, { transaction_hash: TX_HASH })], {
        transactionIndex: 0,
      }),
    /blockReceipts\[0\]\.transactionHash must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildEvmReceiptTrieProofFromReceipts([receipt, fullReceipt(1, { transactionHash: TX_HASH })], {
        transactionIndex: 0,
      }),
    /transactionHash values must be unique/u,
  );
  assert.throws(
    () => buildEvmReceiptTrieProofFromReceipts([receipt], { transactionIndex: 1 }),
    /block receipt index/u,
  );
  assert.throws(
    () => buildEvmReceiptTrieProofFromReceipts([], { transactionIndex: 0 }),
    /non-empty array/u,
  );
  assert.throws(
    () =>
      buildEvmReceiptTrieProofFromReceipts(Array.from({ length: 4_097 }, () => receipt), {
        transactionIndex: 0,
      }),
    /at most/u,
  );
  assert.throws(
    () => canonicalEvmReceiptRlp(fullReceipt(0, { logsBloom: `0x${"AA".repeat(256)}` })),
    /lowercase/u,
  );
  assert.throws(
    () => canonicalEvmReceiptRlp(fullReceipt(0, { type: "0x80" })),
    /below 0x80/u,
  );
  assert.throws(
    () => canonicalEvmReceiptRlp(fullReceipt(0, { type: "0x7f" })),
    /not supported/u,
  );
  assert.throws(
    () => canonicalEvmReceiptRlp(fullReceipt(0, { logs: [sourceEventLog({ removed: true })] })),
    /removed/u,
  );
  assert.throws(
    () =>
      canonicalEvmReceiptRlp(
        fullReceipt(0, {
          logs: [
            sourceEventLog({
              topics: Array.from({ length: 5 }, () => hex32("22")),
            }),
          ],
        }),
      ),
    /topics/u,
  );
});

test("Ethereum receipt-proof transcript rejects empty trie and finality branches", () => {
  assert.throws(
    () =>
      canonicalEvmSccpReceiptProofBytes({
        ...sampleReceiptProof,
        receiptTrieProofNodes: [],
      }),
    /receiptTrieProofNodes must not be empty/u,
  );
  assert.throws(
    () =>
      evmSccpReceiptProofHash({
        ...sampleReceiptProof,
        inclusionBranch: [],
      }),
    /inclusionBranch must not be empty/u,
  );
  assert.throws(
    () =>
      evmSccpReceiptProofHash({
        ...sampleReceiptProof,
        sourceDomain: SCCP_DOMAIN_BSC,
      }),
    /sourceDomain must be ETH/u,
  );
});

test("EthereumMainnetSccp builds receipt proof nodes from user JSON-RPC receipts", async () => {
  const blockReceipts = sampleBlockReceipts();
  const receiptTrieProof = buildEvmReceiptTrieProofFromReceipts(blockReceipts, {
    transactionIndex: 0,
  });
  const calls = [];
  const provider = {
    async request({ method, params }) {
      calls.push([method, params]);
      if (method === "eth_chainId") return "0x1";
      if (method === "eth_getTransactionReceipt") {
        assert.deepEqual(params, [TX_HASH]);
        return blockReceipts[0];
      }
      if (method === "eth_getBlockByHash") {
        assert.deepEqual(params, [BLOCK_HASH, false]);
        return {
          hash: BLOCK_HASH,
          number: "0x1234",
          receiptsRoot: receiptTrieProof.receiptsRoot,
        };
      }
      if (method === "eth_getBlockReceipts") {
        assert.deepEqual(params, ["0x1234"]);
        return blockReceipts;
      }
      throw new Error(`unexpected RPC method ${method}`);
    },
  };
  const sdk = new EthereumMainnetSccp({
    executionProvider: provider,
    sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
    consensusProvider: {
      collectFinalityEvidence() {
        return {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: receiptTrieProof.receiptsRoot,
        };
      },
    },
  });

  const evidence = await sdk.collectInboundEvidenceFromReceipt({
    transactionHash: TX_HASH,
    inclusionBranch: [hex32("f1")],
  });

  assert.equal(evidence.sourceEventDigest, SOURCE_EVENT_DIGEST);
  assert.equal(evidence.receiptProof.receiptRootIndex, "0");
  assert.equal(evidence.receiptProof.executionReceiptsRoot, receiptTrieProof.receiptsRoot);
  assert.deepEqual(
    evidence.receiptProof.receiptTrieProofNodes,
    receiptTrieProof.receiptTrieProofNodes,
  );
  assert.equal(evidence.receiptProofHash, evmSccpReceiptProofHash(evidence.receiptProof));
  for (const [alias, value, label] of [
    ["cumulative_gas_used", "0x5208", "receipt.cumulativeGasUsed"],
    ["logs_bloom", `0x${"11".repeat(256)}`, "receipt.logsBloom"],
  ]) {
    assert.throws(
      () =>
        buildEvmReceiptTrieProofFromReceipts(
          [{ ...blockReceipts[0], [alias]: value }],
          { transactionIndex: 0 },
        ),
      new RegExp(`${label} must not use multiple aliases`, "u"),
    );
  }
  assert.deepEqual(
    calls.map(([method]) => method),
    [
      "eth_chainId",
      "eth_getTransactionReceipt",
      "eth_getBlockByHash",
      "eth_getBlockReceipts",
    ],
  );

  const baseCollectionInput = {
    receipt: blockReceipts[0],
    block: {
      hash: BLOCK_HASH,
      number: "0x1234",
      receiptsRoot: receiptTrieProof.receiptsRoot,
    },
    beaconFinality: {
      finalizedHeaderRoot: hex32("dd"),
      syncCommitteeRoot: hex32("ee"),
      beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
      executionBlockNumber: "0x1234",
      executionBlockHash: BLOCK_HASH,
      executionReceiptsRoot: receiptTrieProof.receiptsRoot,
    },
    blockReceipts,
    inclusionBranch: [hex32("f1")],
  };
  const localSdk = new EthereumMainnetSccp({
    sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
  });
  for (const field of ["finalizedHeaderRoot", "syncCommitteeRoot", "beaconSlot"]) {
    const incompleteFinality = { ...baseCollectionInput.beaconFinality };
    delete incompleteFinality[field];
    await assert.rejects(
      () =>
        localSdk.collectInboundEvidenceFromReceipt({
          ...baseCollectionInput,
          beaconFinality: incompleteFinality,
        }),
      new RegExp(
        `receipt proof construction requires beaconFinality\\.${field}`,
        "u",
      ),
    );
  }
  for (const [alias, value, label] of [
    ["transaction_hash", hex32("ab"), "receipt.transactionHash"],
    ["block_hash", hex32("ab"), "receipt.blockHash"],
    ["block_number", "0x1235", "receipt.blockNumber"],
    ["transaction_index", "0x1", "receipt.transactionIndex"],
  ]) {
    await assert.rejects(
      () =>
        localSdk.collectInboundEvidenceFromReceipt({
          ...baseCollectionInput,
          receipt: { ...blockReceipts[0], [alias]: value },
        }),
      new RegExp(`${label.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&")} must not use multiple aliases`, "u"),
    );
  }
  for (const [alias, value, label] of [
    ["blockNumber", "0x1235", "block.number"],
    ["block_number", "0x1235", "block.number"],
    ["receipts_root", hex32("ab"), "block.receiptsRoot"],
  ]) {
    await assert.rejects(
      () =>
        localSdk.collectInboundEvidenceFromReceipt({
          ...baseCollectionInput,
          block: { ...baseCollectionInput.block, [alias]: value },
        }),
      new RegExp(`${label.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&")} must not use multiple aliases`, "u"),
    );
  }
  for (const [alias, value, label] of [
    ["transaction_hash", hex32("ab"), "blockReceipts[0].transactionHash"],
    ["block_hash", hex32("ab"), "blockReceipts.blockHash"],
    ["block_number", "0x1235", "blockReceipts.blockNumber"],
  ]) {
    await assert.rejects(
      () =>
        localSdk.collectInboundEvidenceFromReceipt({
          ...baseCollectionInput,
          blockReceipts: [{ ...blockReceipts[0], [alias]: value }, blockReceipts[1]],
        }),
      new RegExp(`${label.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&")} must not use multiple aliases`, "u"),
    );
  }

  const mismatchedBlockReceipts = [fullReceipt(0, { logs: [] }), blockReceipts[1]];
  const mismatchedReceiptProof = buildEvmReceiptTrieProofFromReceipts(mismatchedBlockReceipts, {
    transactionIndex: 0,
  });
  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }).collectInboundEvidenceFromReceipt({
        receipt: blockReceipts[0],
        block: {
          hash: BLOCK_HASH,
          number: "0x1234",
          receiptsRoot: mismatchedReceiptProof.receiptsRoot,
        },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: mismatchedReceiptProof.receiptsRoot,
        },
        blockReceipts: mismatchedBlockReceipts,
        inclusionBranch: [hex32("f1")],
      }),
    /receipt RLP/u,
  );

  const blockHashDriftReceipts = [
    fullReceipt(0, { blockHash: hex32("99") }),
    blockReceipts[1],
  ];
  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }).collectInboundEvidenceFromReceipt({
        receipt: blockReceipts[0],
        block: {
          hash: BLOCK_HASH,
          number: "0x1234",
          receiptsRoot: receiptTrieProof.receiptsRoot,
        },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: receiptTrieProof.receiptsRoot,
        },
        blockReceipts: blockHashDriftReceipts,
        inclusionBranch: [hex32("f1")],
      }),
    /blockHash/u,
  );

  const blockNumberDriftReceipts = [
    fullReceipt(0, { blockNumber: "0x1235" }),
    blockReceipts[1],
  ];
  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }).collectInboundEvidenceFromReceipt({
        receipt: blockReceipts[0],
        block: {
          hash: BLOCK_HASH,
          number: "0x1234",
          receiptsRoot: receiptTrieProof.receiptsRoot,
        },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: receiptTrieProof.receiptsRoot,
        },
        blockReceipts: blockNumberDriftReceipts,
        inclusionBranch: [hex32("f1")],
      }),
    /blockNumber/u,
  );
});

test("EthereumMainnetBeaconRestConsensusProvider collects finalized Beacon REST evidence", async () => {
  const fetchCalls = [];
  const beaconFetch = async (url, init) => {
    fetchCalls.push([url, init]);
    if (url === "https://beacon.example/eth/v1/beacon/genesis") {
      return {
        ok: true,
        async json() {
          return { data: { genesis_time: "0" } };
        },
      };
    }
    if (url === "https://beacon.example/eth/v1/beacon/headers/finalized") {
      return {
        ok: true,
        async json() {
          return {
            execution_optimistic: false,
            finalized: true,
            data: {
              root: BEACON_HEADER_ROOT_SLOT_64,
              canonical: true,
              header: {
                message: {
                  slot: "64",
                  proposer_index: "1",
                  parent_root: hex32("01"),
                  state_root: hex32("02"),
                  body_root: hex32("03"),
                },
                signature: `0x${"12".repeat(96)}`,
              },
            },
          };
        },
      };
    }
    if (url === "https://beacon.example/eth/v1/beacon/headers/64") {
      return {
        ok: true,
        async json() {
          return {
            execution_optimistic: false,
            finalized: true,
            data: {
              root: BEACON_HEADER_ROOT_SLOT_64,
              canonical: true,
              header: {
                message: {
                  slot: "64",
                  proposer_index: "1",
                  parent_root: hex32("01"),
                  state_root: hex32("02"),
                  body_root: hex32("03"),
                },
                signature: `0x${"12".repeat(96)}`,
              },
            },
          };
        },
      };
    }
    if (url === "https://beacon.example/eth/v1/beacon/blocks/64/root") {
      return {
        ok: true,
        async json() {
          return {
            execution_optimistic: false,
            finalized: true,
            data: { root: BEACON_HEADER_ROOT_SLOT_64 },
          };
        },
      };
    }
    if (url === "https://beacon.example/eth/v2/beacon/blocks/64") {
      return {
        ok: true,
        async json() {
          return {
            execution_optimistic: false,
            finalized: true,
            data: {
              message: {
                slot: "64",
                body: {
                  execution_payload: {
                    block_hash: BLOCK_HASH,
                    block_number: "4660",
                    receipts_root: hex32("cc"),
                  },
                },
              },
            },
          };
        },
      };
    }
    if (url === "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints") {
      return {
        ok: true,
        async json() {
          return {
            execution_optimistic: false,
            finalized: true,
            data: {
              finalized: { root: BEACON_HEADER_ROOT_SLOT_64, epoch: "2" },
            },
          };
        },
      };
    }
    if (url === "https://beacon.example/eth/v1/beacon/light_client/finality_update") {
      return {
        ok: true,
        async json() {
          return {
            execution_optimistic: false,
            data: {
              finalized_header: {
                beacon: {
                  slot: "64",
                  proposer_index: "1",
                  parent_root: hex32("01"),
                  state_root: hex32("02"),
                  body_root: hex32("03"),
                },
              },
              finality_branch: SAMPLE_FINALITY_BRANCH,
              sync_aggregate: {
                sync_committee_bits: SAMPLE_SYNC_COMMITTEE_BITS,
                sync_committee_signature: `0x${"34".repeat(96)}`,
              },
              signature_slot: "65",
            },
          };
        },
      };
    }
    throw new Error(`unexpected Beacon REST URL ${url}`);
  };
  const executionProvider = {
    async request({ method, params }) {
      if (method === "eth_chainId") return "0x1";
      if (method === "eth_getTransactionReceipt") {
        return {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
        };
      }
      if (method === "eth_getBlockByHash") {
        assert.deepEqual(params, [BLOCK_HASH, false]);
        return {
          hash: BLOCK_HASH,
          number: "0x1234",
          receiptsRoot: hex32("cc"),
          timestamp: "0x300",
        };
      }
      throw new Error(`unexpected RPC method ${method}`);
    },
  };
  const consensusProvider = new EthereumMainnetBeaconRestConsensusProvider({
    endpoint: "https://beacon.example/eth/v1",
    fetch: beaconFetch,
    headers: { authorization: "Bearer local" },
    syncCommitteeRoot: hex32("ee"),
  });
  const sdk = new EthereumMainnetSccp({ executionProvider, consensusProvider });

  const evidence = await sdk.collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH });

  assert.equal(evidence.beaconFinality.executionBlockNumber, "4660");
  assert.equal(evidence.beaconFinality.executionBlockHash, BLOCK_HASH);
  assert.equal(evidence.beaconFinality.executionReceiptsRoot, hex32("cc"));
  assert.equal(evidence.beaconFinality.finalizedHeaderRoot, BEACON_HEADER_ROOT_SLOT_64);
  assert.equal(evidence.beaconFinality.syncCommitteeRoot, hex32("ee"));
  assert.equal(evidence.beaconFinality.beaconSlot, "64");
  assert.deepEqual(evidence.beaconFinality.finalityBranch, SAMPLE_FINALITY_BRANCH);
  assert.equal(evidence.beaconFinality.syncCommitteeBits, SAMPLE_SYNC_COMMITTEE_BITS);
  assert.equal(evidence.beaconFinality.syncCommitteeSignature, `0x${"34".repeat(96)}`);
  assert.equal(evidence.beaconFinality.syncCommitteeParticipation, SAMPLE_SYNC_COMMITTEE_PARTICIPATION);
  assert.equal(evidence.beaconFinality.syncSignatureSlot, "65");
  assert.deepEqual(
    fetchCalls.map(([url]) => url),
    [
      "https://beacon.example/eth/v1/beacon/genesis",
      "https://beacon.example/eth/v1/beacon/headers/finalized",
      "https://beacon.example/eth/v1/beacon/headers/64",
      "https://beacon.example/eth/v1/beacon/blocks/64/root",
      "https://beacon.example/eth/v2/beacon/blocks/64",
      "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints",
      "https://beacon.example/eth/v1/beacon/light_client/finality_update",
    ],
  );
  for (const [, init] of fetchCalls) {
    assert.equal(init.method, "GET");
    assert.equal(init.headers.authorization, "Bearer local");
  }
});

test("EthereumMainnetBeaconRestConsensusProvider rejects unsafe or incomplete Beacon REST data", async () => {
  const block = {
    hash: BLOCK_HASH,
    number: "0x1234",
    receiptsRoot: hex32("cc"),
    beaconSlot: "64",
  };
  const validHeader = () => ({
    execution_optimistic: false,
    finalized: true,
    data: {
      root: BEACON_HEADER_ROOT_SLOT_64,
      canonical: true,
      header: {
        message: {
          slot: "64",
          proposer_index: "1",
          parent_root: hex32("01"),
          state_root: hex32("02"),
          body_root: hex32("03"),
        },
        signature: `0x${"12".repeat(96)}`,
      },
    },
  });
  const validCheckpoint = () => ({
    execution_optimistic: false,
    finalized: true,
    data: { finalized: { root: BEACON_HEADER_ROOT_SLOT_64, epoch: "2" } },
  });
  const validBlockRoot = () => ({
    execution_optimistic: false,
    finalized: true,
    data: { root: BEACON_HEADER_ROOT_SLOT_64 },
  });
  const validBlock = () => ({
    execution_optimistic: false,
    finalized: true,
    data: {
      message: {
        slot: "64",
        body: {
          execution_payload: {
            block_hash: BLOCK_HASH,
            block_number: "4660",
            receipts_root: hex32("cc"),
          },
        },
      },
    },
  });
  const validFinalityUpdate = () => ({
    execution_optimistic: false,
    data: {
      finalized_header: {
        beacon: {
          slot: "64",
          proposer_index: "1",
          parent_root: hex32("01"),
          state_root: hex32("02"),
          body_root: hex32("03"),
        },
      },
      finality_branch: SAMPLE_FINALITY_BRANCH,
      sync_aggregate: {
        sync_committee_bits: SAMPLE_SYNC_COMMITTEE_BITS,
        sync_committee_signature: `0x${"34".repeat(96)}`,
      },
      signature_slot: "65",
    },
  });
  const syncCommitteePayload = {
    syncCommitteePublicKeys: Array.from({ length: 512 }, (_, index) =>
      indexedHexBytes(0x11, 48, index),
    ),
    syncCommitteeWeights: Array.from({ length: 512 }, () => "1"),
    syncCommitteePops: Array.from({ length: 512 }, (_, index) =>
      indexedHexBytes(0x22, 96, index),
    ),
  };
  const providerFor = (
    headerResponse,
    checkpointResponse = { ok: true, json: async () => validCheckpoint() },
    extra = {},
    blockResponse = { ok: true, json: async () => validBlock() },
    blockRootResponse = { ok: true, json: async () => validBlockRoot() },
    targetHeaderResponse = headerResponse,
    finalityUpdateResponse = { ok: true, json: async () => validFinalityUpdate() },
  ) =>
    new EthereumMainnetBeaconRestConsensusProvider({
      endpoint: "https://beacon.example",
      syncCommitteeRoot: hex32("ee"),
      fetch: async (url) => {
        if (url.endsWith("/eth/v1/beacon/headers/finalized")) return headerResponse;
        if (url.endsWith("/eth/v1/beacon/headers/64")) return targetHeaderResponse;
        if (url.endsWith("/eth/v1/beacon/blocks/64/root")) return blockRootResponse;
        if (url.endsWith("/eth/v2/beacon/blocks/64")) return blockResponse;
        if (url.endsWith("/eth/v1/beacon/states/finalized/finality_checkpoints")) {
          return checkpointResponse;
        }
        if (url.endsWith("/eth/v1/beacon/light_client/finality_update")) {
          return finalityUpdateResponse;
        }
        throw new Error(`unexpected Beacon REST URL ${url}`);
      },
      ...extra,
    });
  const streamResponse = (chunks, extra = {}) => ({
    ok: true,
    body: {
      getReader() {
        let index = 0;
        return {
          async read() {
            if (index >= chunks.length) return { done: true };
            const value = chunks[index];
            index += 1;
            return { done: false, value };
          },
          async cancel() {},
          releaseLock() {},
        };
      },
    },
    ...extra,
  });

  assert.throws(
    () =>
      new EthereumMainnetBeaconRestConsensusProvider({
        endpoint: "https://beacon.example",
        verifyFinalityCheckpoint: 0,
      }),
    /verifyFinalityCheckpoint must be a boolean/u,
  );

  await assert.rejects(
    () =>
      providerFor({ ok: true, json: async () => validHeader() }).collectFinalityEvidence(
        { block },
        { verify_finality_checkpoint: "false" },
      ),
    /verifyFinalityCheckpoint must be a boolean/u,
  );

  const unchecked = await providerFor(
    { ok: true, json: async () => validHeader() },
    {
      ok: true,
      json: async () => ({
        ...validCheckpoint(),
        data: { finalized: { root: hex32("99"), epoch: "2" } },
      }),
    },
  ).collectFinalityEvidence({ block }, { verifyFinalityCheckpoint: false });
  assert.equal(unchecked.finalizedHeaderRoot, BEACON_HEADER_ROOT_SLOT_64);

  await assert.rejects(
    () =>
      providerFor({
        ok: true,
        json: async () => {
          const header = validHeader();
          header.data.root = hex32("00");
          return header;
        },
      }).collectFinalityEvidence({ block }),
    /finalizedHeaderRoot must not be zero/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        { ok: true, json: async () => validBlock() },
        {
          ok: true,
          json: async () => ({
            ...validBlockRoot(),
            data: { root: hex32("00") },
          }),
        },
      ).collectFinalityEvidence({ block }),
    /finalizedBlockRoot must not be zero/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        {
          ok: true,
          json: async () => ({
            ...validCheckpoint(),
            data: { finalized: { root: hex32("00"), epoch: "2" } },
          }),
        },
      ).collectFinalityEvidence({ block }),
    /finalizedCheckpointRoot must not be zero/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        { syncCommitteeRoot: hex32("00") },
      ).collectFinalityEvidence({ block }),
    /syncCommitteeRoot must not be zero/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        { ok: true, json: async () => validBlock() },
        {
          ok: true,
          json: async () => ({
            ...validBlockRoot(),
            data: { root: hex32("99") },
          }),
        },
      ).collectFinalityEvidence({ block }),
    /finalized block root must match finalized header root/u,
  );

  await assert.rejects(
    () => providerFor({ ok: true, json: async () => validHeader() }).collectFinalityEvidence({}),
    /requires block/u,
  );

  await assert.rejects(
    () =>
      providerFor({ ok: true, json: async () => validHeader() }).collectFinalityEvidence({
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
      }),
    /requires beaconSlot, beaconBlockRoot, or block\.timestamp/u,
  );

  await assert.rejects(
    () =>
      providerFor({ ok: false, status: 503, statusText: "Unavailable" })
        .collectFinalityEvidence({ block }),
    /request failed 503 Unavailable/u,
  );

  await assert.rejects(
    () =>
      providerFor({ ok: "false", status: 200, json: async () => validHeader() })
        .collectFinalityEvidence({ block }),
    /response ok must be a boolean/u,
  );

  await assert.rejects(
    () =>
      providerFor({ status: "503", json: async () => validHeader() })
        .collectFinalityEvidence({ block }),
    /response status must be an integer/u,
  );

  await assert.rejects(
    () =>
      providerFor({ status: 503, statusText: "Unavailable", json: async () => validHeader() })
        .collectFinalityEvidence({ block }),
    /request failed 503 Unavailable/u,
  );

  const textEvidence = await providerFor(
    { ok: true, text: async () => JSON.stringify(validHeader()) },
    { ok: true, text: async () => JSON.stringify(validCheckpoint()) },
  ).collectFinalityEvidence({ block });
  assert.equal(textEvidence.finalizedHeaderRoot, BEACON_HEADER_ROOT_SLOT_64);

  const streamEvidence = await providerFor(
    streamResponse([Buffer.from(JSON.stringify(validHeader()))]),
    streamResponse([Buffer.from(JSON.stringify(validCheckpoint()))]),
  ).collectFinalityEvidence({ block });
  assert.equal(streamEvidence.finalizedHeaderRoot, BEACON_HEADER_ROOT_SLOT_64);

  await assert.rejects(
    () =>
      providerFor({ ok: true, text: async () => "x".repeat(1024 * 1024 + 1) })
        .collectFinalityEvidence({ block }),
    /response body must be at most/u,
  );

  await assert.rejects(
    () =>
      providerFor(streamResponse([Buffer.alloc(1024 * 1024), Buffer.from("x")]))
        .collectFinalityEvidence({ block }),
    /response body must be at most/u,
  );

  await assert.rejects(
    () =>
      providerFor(streamResponse(["not bytes"]))
        .collectFinalityEvidence({ block }),
    /response body chunks must be bytes/u,
  );

  await assert.rejects(
    () =>
      providerFor({ ok: true, text: async () => JSON.stringify([]) })
        .collectFinalityEvidence({ block }),
    /response JSON must be an object/u,
  );

  await assert.rejects(
    () =>
      providerFor({ ok: true, json: async () => ({ ...validHeader(), execution_optimistic: true }) })
        .collectFinalityEvidence({ block }),
    /must not be execution optimistic/u,
  );

  await assert.rejects(
    () =>
      providerFor({ ok: true, json: async () => ({ ...validHeader(), execution_optimistic: "false" }) })
        .collectFinalityEvidence({ block }),
    /execution_optimistic must be a boolean/u,
  );

  await assert.rejects(
    () =>
      providerFor({ ok: true, json: async () => ({ ...validHeader(), executionOptimistic: 0 }) })
        .collectFinalityEvidence({ block }),
    /executionOptimistic must be a boolean/u,
  );

  await assert.rejects(
    () =>
      providerFor({ ok: true, json: async () => ({ ...validHeader(), finalized: false }) })
        .collectFinalityEvidence({ block }),
    /must be finalized/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        { ok: true, json: async () => validBlock() },
        { ok: true, json: async () => validBlockRoot() },
        {
          ok: true,
          json: async () => ({ ...validHeader(), finalized: false }),
        },
      ).collectFinalityEvidence({ block }),
    /finalized target header must be finalized/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        {
          ok: true,
          json: async () => {
            const header = validHeader();
            header.data.root = hex32("ff");
            header.data.header.message.slot = "96";
            return header;
          },
        },
        { ok: true, json: async () => validCheckpoint() },
        {},
        { ok: true, json: async () => validBlock() },
        { ok: true, json: async () => validBlockRoot() },
        { ok: true, json: async () => validHeader() },
      ).collectFinalityEvidence({ block }),
    /historical target blocks require an ancestry proof/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        {
          ok: true,
          json: async () => ({ ...validBlock(), finalized: false }),
        },
      ).collectFinalityEvidence({ block }),
    /finalized block must be finalized/u,
  );

  await assert.rejects(
    () =>
      providerFor({ ok: true, json: async () => ({ ...validHeader(), finalized: "true" }) })
        .collectFinalityEvidence({ block }),
    /finalized must be a boolean/u,
  );

  await assert.rejects(
    () =>
      providerFor({
        ok: true,
        json: async () => ({
          ...validHeader(),
          data: { ...validHeader().data, canonical: "true" },
        }),
      }).collectFinalityEvidence({ block }),
    /canonical must be a boolean/u,
  );

  for (const field of ["parent_root", "state_root", "body_root"]) {
    await assert.rejects(
      () =>
        providerFor({
          ok: true,
          json: async () => {
            const header = validHeader();
            delete header.data.header.message[field];
            return header;
          },
        }).collectFinalityEvidence({ block }),
      new RegExp(`${field} is required`, "u"),
    );
  }

  await assert.rejects(
    () =>
      providerFor({
        ok: true,
        json: async () => {
          const header = validHeader();
          header.data.header.message.body_root = `0x${"03".repeat(31)}`;
          return header;
        },
      }).collectFinalityEvidence({ block }),
    /body_root must be 32 bytes/u,
  );

  await assert.rejects(
    () =>
      providerFor({
        ok: true,
        json: async () => {
          const header = validHeader();
          delete header.data.header.signature;
          return header;
        },
      }).collectFinalityEvidence({ block }),
    /signature is required/u,
  );

  await assert.rejects(
    () =>
      providerFor({
        ok: true,
        json: async () => {
          const header = validHeader();
          header.data.header.signature = `0x${"12".repeat(95)}`;
          return header;
        },
      }).collectFinalityEvidence({ block }),
    /signature must be 96 bytes/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        {
          ok: true,
          json: async () => ({
            ...validBlock(),
            data: {
              message: {
                ...validBlock().data.message,
                slot: "65",
              },
            },
          }),
        },
      ).collectFinalityEvidence({ block }),
    /finalized block slot must match finalized header slot/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        {
          ok: true,
          json: async () => {
            const payload = validBlock();
            payload.data.message.body.execution_payload.block_hash = hex32("99");
            return payload;
          },
        },
      ).collectFinalityEvidence({ block }),
    /execution payload block_hash must match block.hash/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        {
          ok: true,
          json: async () => {
            const payload = validBlock();
            payload.data.message.body.execution_payload.block_number = "4661";
            return payload;
          },
        },
      ).collectFinalityEvidence({ block }),
    /execution payload block_number must match block.number/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        {
          ok: true,
          json: async () => {
            const payload = validBlock();
            payload.data.message.body.execution_payload.receipts_root = hex32("99");
            return payload;
          },
        },
      ).collectFinalityEvidence({ block }),
    /execution payload receipts_root must match block.receiptsRoot/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        {
          ok: true,
          json: async () => ({
            ...validCheckpoint(),
            data: { finalized: { root: hex32("99"), epoch: "2" } },
          }),
        },
      ).collectFinalityEvidence({ block }),
    /checkpoint root must match finalized header root/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        { ok: true, json: async () => validBlock() },
        { ok: true, json: async () => validBlockRoot() },
        { ok: true, json: async () => validHeader() },
        {
          ok: true,
          json: async () => {
            const update = validFinalityUpdate();
            update.data.sync_aggregate.sync_committee_bits = `0x${"00".repeat(64)}`;
            return update;
          },
        },
      ).collectFinalityEvidence({ block }),
    /sync_committee_bits must contain at least one participant/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        { ok: true, json: async () => validBlock() },
        { ok: true, json: async () => validBlockRoot() },
        { ok: true, json: async () => validHeader() },
        {
          ok: true,
          json: async () => {
            const update = validFinalityUpdate();
            update.data.sync_aggregate.sync_committee_bits = LOW_SYNC_COMMITTEE_BITS;
            return update;
          },
        },
      ).collectFinalityEvidence({ block }),
    /sync_committee_bits must contain Ethereum sync committee supermajority/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        { ok: true, json: async () => validBlock() },
        { ok: true, json: async () => validBlockRoot() },
        { ok: true, json: async () => validHeader() },
        {
          ok: true,
          json: async () => {
            const update = validFinalityUpdate();
            delete update.data.finality_branch;
            return update;
          },
        },
      ).collectFinalityEvidence({ block }),
    /finality_branch is required/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        { ok: true, json: async () => validBlock() },
        { ok: true, json: async () => validBlockRoot() },
        { ok: true, json: async () => validHeader() },
        {
          ok: true,
          json: async () => {
            const update = validFinalityUpdate();
            update.data.finality_branch = SAMPLE_FINALITY_BRANCH.slice(0, 5);
            return update;
          },
        },
      ).collectFinalityEvidence({ block }),
    /finality_branch must contain 6 siblings/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        {},
        { ok: true, json: async () => validBlock() },
        { ok: true, json: async () => validBlockRoot() },
        { ok: true, json: async () => validHeader() },
        {
          ok: true,
          json: async () => {
            const update = validFinalityUpdate();
            update.data.sync_aggregate.sync_committee_signature = `0x${"00".repeat(96)}`;
            return update;
          },
        },
      ).collectFinalityEvidence({ block }),
    /sync_committee_signature must not be zero/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        { syncCommitteeRoot: null },
      ).collectFinalityEvidence({ block }),
    /requires syncCommitteeRoot or syncCommitteePayload/u,
  );

  await assert.rejects(
    () =>
      providerFor(
        { ok: true, json: async () => validHeader() },
        { ok: true, json: async () => validCheckpoint() },
        { syncCommitteePayload },
      ).collectFinalityEvidence({ block }),
    /syncCommitteeRoot must match syncCommitteePayload/u,
  );
});

test("EthereumMainnetSccp proves only after collecting finality-bound evidence", async () => {
  let proveCalls = 0;
  const sdk = new EthereumMainnetSccp({
    executionProvider: {
      async request({ method }) {
        if (method === "eth_chainId") return "0x1";
        if (method === "eth_getTransactionReceipt") {
          return {
            transactionHash: TX_HASH,
            blockHash: BLOCK_HASH,
            blockNumber: "0x1234",
            status: "0x1",
            logs: [sourceEventLog()],
          };
        }
        if (method === "eth_getBlockByHash") {
          return { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") };
        }
        throw new Error(`unexpected RPC method ${method}`);
      },
    },
    sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
    consensusProvider: {
      collectFinalityEvidence() {
        return {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
          ...sampleFinalityUpdateFields(),
        };
      },
    },
    proveInbound(evidence) {
      proveCalls += 1;
      assert.equal(evidence.transactionHash, TX_HASH);
      assert.equal(evidence.beaconFinality.executionBlockHash, BLOCK_HASH);
      assert.equal(evidence.beaconFinality.beaconSlot, "64");
      assert.equal(evidence.receiptProofHash, evmSccpReceiptProofHash(sampleReceiptProof));
      assert.equal(evidence.sourceEventDigest, SOURCE_EVENT_DIGEST);
      return [1, 2, 3];
    },
  });

  assert.deepEqual(
    await sdk.proveInboundToSora({ transactionHash: TX_HASH, receiptProof: sampleReceiptProof }),
    new Uint8Array([1, 2, 3]),
  );
  assert.equal(proveCalls, 1);

  const proofReadyInput = {
    receipt: {
      transactionHash: TX_HASH,
      blockHash: BLOCK_HASH,
      blockNumber: "0x1234",
      status: "0x1",
      logs: [sourceEventLog()],
    },
    block: {
      hash: BLOCK_HASH,
      number: "0x1234",
      receiptsRoot: sampleReceiptProof.executionReceiptsRoot,
    },
    beaconFinality: {
      finalizedHeaderRoot: sampleReceiptProof.beaconFinalizedRoot,
      syncCommitteeRoot: sampleReceiptProof.syncCommitteeRoot,
      beaconSlot: "0x40",
      executionBlockNumber: "0x1234",
      executionBlockHash: sampleReceiptProof.executionBlockHash,
      executionReceiptsRoot: sampleReceiptProof.executionReceiptsRoot,
      ...sampleFinalityUpdateFields(),
    },
    receiptProof: sampleReceiptProof,
    receiptProofHash: evmSccpReceiptProofHash(sampleReceiptProof),
    sourceEventDigest: SOURCE_EVENT_DIGEST,
    sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
  };
  const oversizedProofBytes = new Uint8Array(SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1).fill(1);
  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          return oversizedProofBytes;
        },
      }).proveInboundToSora(proofReadyInput),
    /proofBytes must be at most/u,
  );
  let submitterCalled = false;
  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        submitInboundToIroha() {
          submitterCalled = true;
        },
      }).submitInboundToIroha(oversizedProofBytes),
    /proofBytes must be at most/u,
  );
  assert.equal(submitterCalled, false);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "64",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
          syncCommitteeBits: SAMPLE_SYNC_COMMITTEE_BITS,
          syncCommitteeSignature: `0x${"00".repeat(96)}`,
          syncCommitteeParticipation: SAMPLE_SYNC_COMMITTEE_PARTICIPATION,
          syncSignatureSlot: "65",
        },
        receiptProof: sampleReceiptProof,
        receiptProofHash: evmSccpReceiptProofHash(sampleReceiptProof),
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.syncCommitteeSignature must not be zero/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "64",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
          syncCommitteeBits: SAMPLE_SYNC_COMMITTEE_BITS,
          syncCommitteeSignature: SAMPLE_SYNC_COMMITTEE_SIGNATURE,
          syncCommitteeParticipation: SAMPLE_SYNC_COMMITTEE_PARTICIPATION,
          syncSignatureSlot: "63",
        },
        receiptProof: sampleReceiptProof,
        receiptProofHash: evmSccpReceiptProofHash(sampleReceiptProof),
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.syncSignatureSlot must cover beaconFinality\.beaconSlot/u,
  );
  assert.equal(proveCalls, 1);

  let aliasOnlyProverCalls = 0;
  assert.deepEqual(
    await new EthereumMainnetSccp({
      proveInbound(evidence) {
        aliasOnlyProverCalls += 1;
        assert.equal(evidence.beaconFinality.executionBlockNumber, "4660");
        assert.equal(evidence.beaconFinality.executionBlockHash, BLOCK_HASH);
        assert.equal(evidence.beaconFinality.executionReceiptsRoot, hex32("cc"));
        assert.equal(evidence.beaconFinality.finalizedHeaderRoot, hex32("dd"));
        assert.equal(evidence.beaconFinality.syncCommitteeRoot, hex32("ee"));
        assert.equal(evidence.beaconFinality.beaconSlot, "64");
        assert.deepEqual(evidence.beaconFinality.finalityBranch, SAMPLE_FINALITY_BRANCH);
        assert.equal(evidence.beaconFinality.syncCommitteeBits, SAMPLE_SYNC_COMMITTEE_BITS);
        assert.equal(evidence.beaconFinality.syncCommitteeSignature, SAMPLE_SYNC_COMMITTEE_SIGNATURE);
        assert.equal(evidence.beaconFinality.syncCommitteeParticipation, SAMPLE_SYNC_COMMITTEE_PARTICIPATION);
        assert.equal(evidence.beaconFinality.syncSignatureSlot, "65");
        assert.equal(evidence.beaconFinality.extensionWitness, "kept");
        for (const alias of [
          "execution_block_number",
          "finalityHeight",
          "finality_block_hash",
          "receipts_root",
          "finalized_header_root",
          "sync_committee_root",
          "beacon_slot",
          "finality_branch",
          "sync_committee_bits",
          "sync_committee_signature",
          "sync_committee_participation",
          "signature_slot",
        ]) {
          assert.equal(alias in evidence.beaconFinality, false);
        }
        return [4, 5, 6];
      },
    }).proveInboundToSora({
      receipt: {
        transactionHash: TX_HASH,
        blockHash: BLOCK_HASH,
        blockNumber: "0x1234",
        status: "0x1",
        logs: [sourceEventLog()],
      },
      block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
      beaconFinality: {
        execution_block_number: "0x1234",
        finality_block_hash: BLOCK_HASH,
        receipts_root: hex32("cc"),
        finalized_header_root: hex32("dd"),
        sync_committee_root: hex32("ee"),
        beacon_slot: "0x40",
        finality_branch: SAMPLE_FINALITY_BRANCH,
        sync_committee_bits: SAMPLE_SYNC_COMMITTEE_BITS,
        sync_committee_signature: SAMPLE_SYNC_COMMITTEE_SIGNATURE,
        sync_committee_participation: SAMPLE_SYNC_COMMITTEE_PARTICIPATION,
        signature_slot: "65",
        extensionWitness: "kept",
      },
      receiptProof: sampleReceiptProof,
      receiptProofHash: evmSccpReceiptProofHash(sampleReceiptProof),
      sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
    }),
    new Uint8Array([4, 5, 6]),
  );
  assert.equal(aliasOnlyProverCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
          syncCommitteeBits: SAMPLE_SYNC_COMMITTEE_BITS,
          syncCommitteeSignature: SAMPLE_SYNC_COMMITTEE_SIGNATURE,
          syncCommitteeParticipation: "1",
          syncSignatureSlot: "65",
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.syncCommitteeParticipation must match syncCommitteeBits/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
          syncCommitteeBits: SAMPLE_SYNC_COMMITTEE_BITS,
          syncCommitteeSignature: SAMPLE_SYNC_COMMITTEE_SIGNATURE,
          syncCommitteeParticipation: SAMPLE_SYNC_COMMITTEE_PARTICIPATION,
          syncSignatureSlot: "65",
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.finalityBranch/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
          syncCommitteeBits: LOW_SYNC_COMMITTEE_BITS,
          syncCommitteeSignature: SAMPLE_SYNC_COMMITTEE_SIGNATURE,
          syncCommitteeParticipation: "1",
          syncSignatureSlot: "65",
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.syncCommitteeBits must contain Ethereum sync committee supermajority/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("00"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.finalizedHeaderRoot must not be zero/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: hex32("00"),
          executionReceiptsRoot: hex32("cc"),
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.executionBlockHash must not be zero/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("00"),
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.executionReceiptsRoot must not be zero/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("00"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.syncCommitteeRoot must not be zero/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
        },
        receiptProof: sampleReceiptProof,
      }),
    /requires receipt source event validation/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({ sourceDomain: SCCP_DOMAIN_ETH, targetDomain: SCCP_DOMAIN_SORA }),
    /requires a receipt, receiptProof, receiptProofHash, or transactionHash/u,
  );
  assert.equal(proveCalls, 1);

  const receiptProofHash = evmSccpReceiptProofHash(sampleReceiptProof);
  const receiptProofEvidence = await new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({
    receiptProof: sampleReceiptProof,
    receiptProofHash,
  });
  assert.equal(receiptProofEvidence.receiptProofHash, receiptProofHash);
  const receiptProofHashOnlyEvidence = await new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({
    receipt_proof_hash: receiptProofHash,
  });
  assert.equal(receiptProofHashOnlyEvidence.receiptProofHash, receiptProofHash);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({
        receiptProofHash: hex32("00"),
      }),
    /receiptProofHash must not be zero/u,
  );

  for (const [field, pattern] of [
    ["sourceEventDigest", /sourceEventDigest must not be zero/u],
    ["executionBlockHash", /executionBlockHash must not be zero/u],
    ["executionReceiptsRoot", /executionReceiptsRoot must not be zero/u],
    ["beaconFinalizedRoot", /beaconFinalizedRoot must not be zero/u],
    ["syncCommitteeRoot", /syncCommitteeRoot must not be zero/u],
  ]) {
    await assert.rejects(
      () =>
        new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({
          receiptProof: { ...sampleReceiptProof, [field]: hex32("00") },
        }),
      pattern,
    );
  }

  const matchingBeaconFinality = {
    finalizedHeaderRoot: sampleReceiptProof.beaconFinalizedRoot,
    syncCommitteeRoot: sampleReceiptProof.syncCommitteeRoot,
    beaconSlot: "0x40",
    executionBlockNumber: "0x1234",
    executionBlockHash: sampleReceiptProof.executionBlockHash,
    executionReceiptsRoot: sampleReceiptProof.executionReceiptsRoot,
    ...sampleFinalityUpdateFields(),
  };
  for (const [field, value, pattern] of [
    [
      "executionBlockNumber",
      "0x1235",
      /receiptProof\.executionBlockNumber must match beaconFinality\.executionBlockNumber/u,
    ],
    [
      "executionBlockHash",
      hex32("99"),
      /receiptProof\.executionBlockHash must match beaconFinality\.executionBlockHash/u,
    ],
    [
      "executionReceiptsRoot",
      hex32("99"),
      /receiptProof\.executionReceiptsRoot must match beaconFinality\.executionReceiptsRoot/u,
    ],
    [
      "finalizedHeaderRoot",
      hex32("99"),
      /receiptProof\.beaconFinalizedRoot must match beaconFinality\.finalizedHeaderRoot/u,
    ],
    [
      "syncCommitteeRoot",
      hex32("99"),
      /receiptProof\.syncCommitteeRoot must match beaconFinality\.syncCommitteeRoot/u,
    ],
    [
      "beaconSlot",
      "0x41",
      /receiptProof\.beaconSlot must match beaconFinality\.beaconSlot/u,
    ],
  ]) {
    await assert.rejects(
      () =>
        new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({
          receiptProof: sampleReceiptProof,
          beaconFinality: { ...matchingBeaconFinality, [field]: value },
        }),
      pattern,
    );
  }

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }).collectInboundEvidenceFromReceipt({
        receipt: fullReceipt(0),
        block: {
          hash: BLOCK_HASH,
          number: "0x1234",
          receiptsRoot: sampleReceiptProof.executionReceiptsRoot,
        },
        beaconFinality: matchingBeaconFinality,
        receiptProof: { ...sampleReceiptProof, sourceEventDigest: hex32("99") },
      }),
    /receiptProof\.sourceEventDigest must match receipt source event/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({
        receipt_proof_hash: `${receiptProofHash} `,
      }),
    /receiptProofHash must be canonical hex/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({
        receiptProof: sampleReceiptProof,
        sourceEventDigest: SOURCE_EVENT_DIGEST,
      }),
    /source event validation requires receipt logs/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({
        receiptProof: sampleReceiptProof,
        receiptProofHash: hex32("99"),
      }),
    /receiptProofHash must match receiptProof/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
        },
        receiptProof: sampleReceiptProof,
        receiptProofHash,
      }),
    /requires receipt source event validation/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () => new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /executionProvider is required/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
      }),
    /requires beaconFinality/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
        },
        receiptProofHash: evmSccpReceiptProofHash(sampleReceiptProof),
      }),
    /requires receiptProof/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
        },
        receiptProof: { ...sampleReceiptProof, executionReceiptsRoot: hex32("99") },
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /receiptProof\.executionReceiptsRoot/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.finalizedHeaderRoot/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.syncCommitteeRoot/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.beaconSlot/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
          syncCommitteeSignature: SAMPLE_SYNC_COMMITTEE_SIGNATURE,
          syncCommitteeParticipation: "1",
          syncSignatureSlot: "65",
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /beaconFinality\.syncCommitteeBits/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("99"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /receiptProof\.beaconFinalizedRoot/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("99"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
        },
        receiptProof: sampleReceiptProof,
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /receiptProof\.syncCommitteeRoot/u,
  );
  assert.equal(proveCalls, 1);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        proveInbound() {
          proveCalls += 1;
          return [1, 2, 3];
        },
      }).proveInboundToSora({
        receipt: {
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
          logs: [sourceEventLog()],
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
        beaconFinality: {
          finalizedHeaderRoot: hex32("dd"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
          finalityBranch: SAMPLE_FINALITY_BRANCH,
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
        },
        receiptProof: { ...sampleReceiptProof, beaconSlot: "65" },
        sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
      }),
    /receiptProof\.beaconSlot/u,
  );
  assert.equal(proveCalls, 1);
});

test("EthereumMainnetSccp inbound prover receives immutable evidence snapshots", async () => {
  const mutableReceiptProof = {
    ...sampleReceiptProof,
    receiptTrieProofNodes: sampleReceiptProof.receiptTrieProofNodes.map((node) => [...node]),
    inclusionBranch: [...sampleReceiptProof.inclusionBranch],
  };
  const mutableBeaconFinality = {
    finalizedHeaderRoot: sampleReceiptProof.beaconFinalizedRoot,
    syncCommitteeRoot: sampleReceiptProof.syncCommitteeRoot,
    beaconSlot: "0x40",
    executionBlockNumber: "0x1234",
    executionBlockHash: sampleReceiptProof.executionBlockHash,
    executionReceiptsRoot: sampleReceiptProof.executionReceiptsRoot,
    ...sampleFinalityUpdateFields(),
    finalityBranch: [...SAMPLE_FINALITY_BRANCH],
  };
  const receipt = fullReceipt(0);
  const sdk = new EthereumMainnetSccp({
    sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
    proveInbound(evidence) {
      assert.equal(Object.isFrozen(evidence), true);
      assert.equal(Object.isFrozen(evidence.receipt), true);
      assert.equal(Object.isFrozen(evidence.receipt.logs), true);
      assert.equal(Object.isFrozen(evidence.receipt.logs[0]), true);
      assert.equal(Object.isFrozen(evidence.receipt.logs[0].topics), true);
      assert.equal(Object.isFrozen(evidence.receiptProof), true);
      assert.equal(Object.isFrozen(evidence.receiptProof.receiptTrieProofNodes), true);
      assert.equal(Object.isFrozen(evidence.receiptProof.receiptTrieProofNodes[0]), true);
      assert.equal(Object.isFrozen(evidence.beaconFinality), true);
      assert.equal(Object.isFrozen(evidence.beaconFinality.finalityBranch), true);
      assert.notEqual(evidence.receipt, receipt);
      assert.notEqual(evidence.receiptProof, mutableReceiptProof);
      assert.notEqual(
        evidence.receiptProof.receiptTrieProofNodes[0],
        mutableReceiptProof.receiptTrieProofNodes[0],
      );
      assert.notEqual(evidence.beaconFinality, mutableBeaconFinality);
      assert.notEqual(evidence.beaconFinality.finalityBranch, mutableBeaconFinality.finalityBranch);
      assert.throws(
        () => {
          evidence.receipt.logs[0].topics.push(hex32("99"));
        },
        TypeError,
      );
      assert.throws(
        () => {
          evidence.receiptProof.receiptTrieProofNodes[0].push(0xff);
        },
        TypeError,
      );
      assert.throws(
        () => {
          evidence.beaconFinality.finalityBranch.push(hex32("99"));
        },
        TypeError,
      );
      return [1, 2, 3];
    },
  });

  assert.deepEqual(
    await sdk.proveInboundToSora({
      receipt,
      block: {
        hash: BLOCK_HASH,
        number: "0x1234",
        receiptsRoot: sampleReceiptProof.executionReceiptsRoot,
      },
      beaconFinality: mutableBeaconFinality,
      receiptProof: mutableReceiptProof,
    }),
    new Uint8Array([1, 2, 3]),
  );
  assert.equal(
    mutableReceiptProof.receiptTrieProofNodes[0][0],
    sampleReceiptProof.receiptTrieProofNodes[0][0],
  );
  assert.deepEqual(mutableBeaconFinality.finalityBranch, SAMPLE_FINALITY_BRANCH);
  assert.deepEqual(receipt.logs[0].topics, [evmSccpSourceEventTopic(), SOURCE_EVENT_DIGEST]);
});

test("EthereumMainnetSccp rejects failed or drifted receipt evidence before proving", async () => {
  const providerForReceipt = (receipt, block = { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") }) => ({
    async request({ method }) {
      if (method === "eth_chainId") return "0x1";
      if (method === "eth_getTransactionReceipt") return receipt;
      if (method === "eth_getBlockByHash") return block;
      throw new Error(`unexpected RPC method ${method}`);
    },
  });

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt({
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x0",
        }),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /receipt status must be 0x1/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt({
          transactionHash: hex32("ab"),
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
        }),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /transactionHash must match/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt({
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          status: "0x1",
        }),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /receipt\.blockNumber is required/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt({
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x0",
          status: "0x1",
        }),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /receipt\.blockNumber must be positive/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt(
          {
            transactionHash: TX_HASH,
            blockHash: BLOCK_HASH,
            blockNumber: "0x1234",
            status: "0x1",
          },
          { hash: hex32("bc"), number: "0x1234", receiptsRoot: hex32("cc") },
        ),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /block.hash must match receipt.blockHash/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt(
          {
            transactionHash: TX_HASH,
            blockHash: BLOCK_HASH,
            blockNumber: "0x1234",
            status: "0x1",
          },
          { hash: BLOCK_HASH, receiptsRoot: hex32("cc") },
        ),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /block\.number is required/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt(
          {
            transactionHash: TX_HASH,
            blockHash: BLOCK_HASH,
            blockNumber: "0x1234",
            status: "0x1",
          },
          { hash: BLOCK_HASH, number: "0x0", receiptsRoot: hex32("cc") },
        ),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /block\.number must be positive/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt({
          transactionHash: TX_HASH.toUpperCase(),
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
        }),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /canonical lowercase/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({
        transactionHash: hex32("00"),
      }),
    /transactionHash must not be zero/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({
        blockHash: hex32("00"),
        receiptProofHash: evmSccpReceiptProofHash(sampleReceiptProof),
      }),
    /blockHash must not be zero/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt({
          transactionHash: hex32("00"),
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
        }),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /receipt\.transactionHash must not be zero/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt({
          transactionHash: TX_HASH,
          blockHash: hex32("00"),
          blockNumber: "0x1234",
          status: "0x1",
        }),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /receipt\.blockHash must not be zero/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt(
          {
            transactionHash: TX_HASH,
            blockHash: BLOCK_HASH,
            blockNumber: "0x1234",
            status: "0x1",
          },
          { hash: hex32("00"), number: "0x1234", receiptsRoot: hex32("cc") },
        ),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /block\.hash must not be zero/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt(
          {
            transactionHash: TX_HASH,
            blockHash: BLOCK_HASH,
            blockNumber: "0x1234",
            status: "0x1",
          },
          { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("00") },
        ),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /block\.receiptsRoot must not be zero/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt({
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
        }),
        consensusProvider: {
          collectFinalityEvidence() {
            return {
              executionBlockNumber: "0x1234",
              executionBlockHash: hex32("bc"),
              executionReceiptsRoot: hex32("cc"),
            };
          },
        },
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /beaconFinality\.executionBlockHash must match block\.hash/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt({
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
        }),
        consensusProvider: {
          collectFinalityEvidence() {
            return {
              executionBlockNumber: "0x1235",
              executionBlockHash: BLOCK_HASH,
              executionReceiptsRoot: hex32("cc"),
            };
          },
        },
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /beaconFinality\.executionBlockNumber must match block\.number/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        executionProvider: providerForReceipt({
          transactionHash: TX_HASH,
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
        }),
        consensusProvider: {
          collectFinalityEvidence() {
            return {
              executionBlockNumber: "0x1234",
              executionBlockHash: BLOCK_HASH,
              executionReceiptsRoot: hex32("cd"),
            };
          },
        },
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /beaconFinality\.executionReceiptsRoot must match block\.receiptsRoot/u,
  );
});

test("EthereumMainnetSccp validates source bridge logs in receipt evidence", async () => {
  const receipt = {
    transactionHash: TX_HASH,
    blockHash: BLOCK_HASH,
    blockNumber: "0x1234",
    status: "0x1",
    logs: [
      {
        address: `0x${"00".repeat(20)}`,
        topics: [hex32("00")],
        data: "0x1234",
      },
      sourceEventLog(),
    ],
  };
  const block = { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") };
  const sdk = new EthereumMainnetSccp({
    sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
  });

  const evidence = await sdk.collectInboundEvidenceFromReceipt({ receipt, block });
  assert.equal(evidence.sourceEventDigest, SOURCE_EVENT_DIGEST);
  assert.equal(evidence.sourceBridgeEmitterAddress, SOURCE_BRIDGE_ADDRESS);

  const explicitEvidence = await sdk.collectInboundEvidenceFromReceipt({
    receipt,
    block,
    sourceEventDigest: SOURCE_EVENT_DIGEST,
  });
  assert.equal(explicitEvidence.sourceEventDigest, SOURCE_EVENT_DIGEST);

  await assert.rejects(
    () =>
      new EthereumMainnetSccp().collectInboundEvidenceFromReceipt({
        receipt,
        block,
        sourceEventDigest: SOURCE_EVENT_DIGEST,
      }),
    /sourceBridgeEmitterAddress is required/u,
  );

  await assert.rejects(
    () =>
      new EthereumMainnetSccp({
        sourceBridgeEmitterAddress: `0x${"45".repeat(20)}`,
      }).collectInboundEvidenceFromReceipt({ receipt, block }),
    /expected SCCP source event/u,
  );

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        receipt: {
          ...receipt,
          logs: [sourceEventLog({ topics: [hex32("99"), SOURCE_EVENT_DIGEST] })],
        },
        block,
      }),
    /expected SCCP source event/u,
  );

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        receipt: {
          ...receipt,
          logs: [
            sourceEventLog({
              topics: [evmSccpSourceEventTopic(), SOURCE_EVENT_DIGEST, hex32("66")],
            }),
          ],
        },
        block,
      }),
    /exactly 2 topics/u,
  );

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        receipt: { ...receipt, logs: [sourceEventLog({ data: "0x01" })] },
        block,
      }),
    /source event log data must be 0x/u,
  );

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        receipt: {
          ...receipt,
          logs: [
            sourceEventLog({ topics: [evmSccpSourceEventTopic(), hex32("00")] }),
          ],
        },
        block,
      }),
    /source event digest must not be zero/u,
  );

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        receipt: { ...receipt, logs: [sourceEventLog(), sourceEventLog()] },
        block,
      }),
    /exactly one matching/u,
  );

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        receipt: { ...receipt, logs: [sourceEventLog({ removed: true })] },
        block,
      }),
    /removed logs/u,
  );

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        receipt: { ...receipt, logs: ["not-a-log"] },
        block,
      }),
    /receipt\.logs\[0\] must be an object/u,
  );

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        receipt: { ...receipt, logs: [sourceEventLog({ data: undefined })] },
        block,
      }),
    /receipt\.logs\[0\]\.data is required/u,
  );

  for (const missingField of ["transactionHash", "blockHash", "blockNumber"]) {
    const log = sourceEventLog();
    delete log[missingField];
    await assert.rejects(
      () =>
        sdk.collectInboundEvidenceFromReceipt({
          receipt: { ...receipt, logs: [log] },
          block,
        }),
      new RegExp(`receipt\\.logs\\[0\\]\\.${missingField}`, "u"),
    );
  }

  for (const [alias, value, label] of [
    ["transaction_hash", hex32("ab"), "receipt.logs[0].transactionHash"],
    ["block_hash", hex32("ac"), "receipt.logs[0].blockHash"],
    ["block_number", "0x1235", "receipt.logs[0].blockNumber"],
  ]) {
    const escapedLabel = label.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
    await assert.rejects(
      () =>
        sdk.collectInboundEvidenceFromReceipt({
          receipt: { ...receipt, logs: [sourceEventLog({ [alias]: value })] },
          block,
        }),
      new RegExp(`${escapedLabel} must not use multiple aliases`, "u"),
    );
  }

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        receipt: { ...receipt, logs: [sourceEventLog({ transactionHash: hex32("ab") })] },
        block,
      }),
    /receipt\.logs transactionHash must match receipt\.transactionHash/u,
  );

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        receipt: { ...receipt, logs: [sourceEventLog({ blockHash: hex32("ab") })] },
        block,
      }),
    /receipt\.logs blockHash must match receipt\.blockHash/u,
  );

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        receipt: { ...receipt, logs: [sourceEventLog({ blockNumber: "0x1235" })] },
        block,
      }),
    /receipt\.logs blockNumber must match receipt\.blockNumber/u,
  );
});

test("EthereumMainnetSccp keeps the easy outbound path Ethereum-only", () => {
  const sdk = new EthereumMainnetSccp();
  const ethRequest = sdk.buildOutboundProofRequest(sampleOutboundInput());
  assert.equal(ethRequest.targetDomain, SCCP_DOMAIN_ETH);
  assert.equal(ethRequest.sourceDomain, SCCP_DOMAIN_SORA);
  assert.equal(ethRequest.destinationBinding.sourceDomain, SCCP_DOMAIN_SORA);
  assert.equal(ethRequest.destinationBinding.networkId, SCCP_ETH_MAINNET_NETWORK_ID);

  assert.throws(
    () => sdk.buildOutboundProofRequest(sampleOutboundInput(SCCP_DOMAIN_BSC)),
    /request route|targetDomain|Ethereum mainnet/u,
  );
  assert.throws(
    () =>
      sdk.buildOutboundProofRequest({
        ...sampleOutboundInput(),
        sourceDomain: SCCP_DOMAIN_BSC,
      }),
    /SORA/u,
  );
});

test("Ethereum outbound prover callback must not see BSC requests", async () => {
  let outboundProverCalled = false;
  const sdk = new EthereumMainnetSccp({
    outboundProver: {
      async prove(request) {
        outboundProverCalled = true;
        return wrapEvmSccpProofResult(groth16ProofBytes(request.publicInputs), request);
      },
    },
  });

  await assert.rejects(
    () => sdk.proveOutboundToEthereum(sampleOutboundInput(SCCP_DOMAIN_BSC)),
    /request route|targetDomain|Ethereum mainnet/u,
  );
  assert.equal(outboundProverCalled, false);
});

test("EthereumMainnetSccp requires linked local prover functions", async () => {
  let executionRequests = 0;
  let outboundProverCalled = false;
  const sdk = new EthereumMainnetSccp({
    executionProvider: {
      async request() {
        executionRequests += 1;
        throw new Error("unexpected execution-provider fallback");
      },
    },
    outboundProver: {
      async prove() {
        outboundProverCalled = true;
        return GROTH16_PROOF_BYTES;
      },
    },
    sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
  });

  await assert.rejects(
    () => sdk.proveInboundToSora({ transactionHash: TX_HASH, receiptProof: sampleReceiptProof }),
    (error) => {
      assert.equal(error.code, "ERR_SCCP_ETH_INBOUND_PROVER_UNAVAILABLE");
      assert.match(error.message, /local JS\/native proveInbound/u);
      return true;
    },
  );
  assert.equal(executionRequests, 0);

  await assert.rejects(
    () => sdk.proveOutboundToEthereum(sampleOutboundInput()),
    (error) => {
      assert.equal(error.code, "ERR_SCCP_ETH_NATIVE_PROVER_ARTIFACTS_UNAVAILABLE");
      assert.match(error.message, /verified native EVM prover artifacts/u);
      return true;
    },
  );
  assert.equal(outboundProverCalled, false);
  assert.equal(executionRequests, 0);
});

test("EthereumMainnetSccp calldata requires a wrapped Ethereum mainnet proof result", () => {
  const { destinationBinding, nativeProverArtifacts } = sampleVerifiedNativeEvmProverFixture();
  const input = { ...sampleOutboundInput(), destinationBinding };
  const sdk = new EthereumMainnetSccp({ nativeProverArtifacts });
  const request = sdk.buildOutboundProofRequest(input);
  const proofResult = wrapEvmSccpProofResult(
    groth16ProofBytes(request.publicInputs),
    request,
  );
  const submission = sdk.buildEthereumCalldata({ proofResult });

  assert.equal(submission.targetDomain, SCCP_DOMAIN_ETH);
  assert.equal(submission.destinationBindingHash, request.destinationBindingHash);
  const tamperedEthereumBase64ProofResult = {
    ...proofResult,
    proofBase64: "AAAA",
  };
  assert.throws(
    () => sdk.buildEthereumCalldata({ proofResult: tamperedEthereumBase64ProofResult }),
    /proofResult\.proofBase64 must match proofResult\.proofBytes/u,
  );
  assert.throws(
    () => new EthereumMainnetSccp().buildEthereumCalldata({ proofResult }),
    /verified native EVM prover artifacts/u,
  );

  assert.throws(
    () =>
      sdk.buildEthereumCalldata({
        publicInputs: samplePublicInputs,
        proofBytes: groth16ProofBytes(samplePublicInputs),
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
      }),
    /wrapped proofResult/u,
  );
  assert.throws(
    () =>
      sdk.buildEthereumCalldata({
        proofResult: {
          ...proofResult,
          destinationBinding: {
            ...proofResult.destinationBinding,
            networkId: hex32("33"),
          },
        },
      }),
    /chain id 1|destinationBinding/u,
  );
});

test("EthereumMainnetSccp rejects malformed Ethereum Groth16 proof tuples", () => {
  const { destinationBinding } = sampleVerifiedNativeEvmProverFixture();
  const request = new EthereumMainnetSccp().buildOutboundProofRequest({
    ...sampleOutboundInput(),
    destinationBinding,
  });
  const rejectProofBytes = (proofBytes, pattern) => {
    assert.throws(() => wrapEvmSccpProofResult(proofBytes, request), pattern);
  };

  rejectProofBytes(groth16ProofBytesWithWord(0, abiWord(2)), /proofBytes\.version/u);
  rejectProofBytes(
    groth16ProofBytesWithWord(4, new Uint8Array(32).fill(0xff)),
    /BN254 base-field/u,
  );
  rejectProofBytes(
    (() => {
      const proofBytes = new Uint8Array(GROTH16_PROOF_BYTES);
      proofBytes.fill(0, 6 * 32, 10 * 32);
      return proofBytes;
    })(),
    /proofBytes\.b/u,
  );
  rejectProofBytes(groth16ProofBytesWithWord(11, abiWord(3)), /proofBytes\.c/u);
  rejectProofBytes(
    groth16ProofBytesWithWord(1, Uint8Array.from({ length: 32 }, () => 0x12)),
    /messageId must match/u,
  );
  rejectProofBytes(
    groth16ProofBytesWithWord(2, abiWord(BigInt(SCCP_DOMAIN_SORA) + 1n)),
    /sourceDomain must match/u,
  );
  rejectProofBytes(
    groth16ProofBytesWithWord(3, Uint8Array.from({ length: 32 }, () => 0x44)),
    /commitmentRoot must match/u,
  );
});

test("EthereumMainnetSccp binds custom outbound proof results to the requested proof", async () => {
  const { destinationBinding, nativeProverArtifacts } = sampleVerifiedNativeEvmProverFixture();
  const input = {
    ...sampleOutboundInput(),
    destinationBinding,
  };
  const referenceSdk = new EthereumMainnetSccp({ nativeProverArtifacts });
  const expectedRequest = referenceSdk.buildOutboundProofRequest(input);
  const wrongFixture = buildSampleOutboundBundleFixture({ nonce: 2n });
  const wrongRequest = referenceSdk.buildOutboundProofRequest({
    ...input,
    publicInputs: wrongFixture.publicInputs,
    bundleBytes: wrongFixture.bundleBytes,
  });
  const wrongProofResult = {
    ...wrapEvmSccpProofResult(
      groth16ProofBytes(expectedRequest.publicInputs),
      expectedRequest,
    ),
    requestHash: wrongRequest.requestHash,
  };
  let seenRequest;
  const rejectingSdk = new EthereumMainnetSccp({
    nativeProverArtifacts,
    nativeProverSelfTest(context) {
      return context.expectedResult;
    },
    outboundProver: {
      async prove(request) {
        seenRequest = request;
        return wrongProofResult;
      },
    },
  });

  assert.notEqual(wrongRequest.requestHash, expectedRequest.requestHash);
  await assert.rejects(
    () => rejectingSdk.proveOutboundToEthereum(input),
    /requestHash must match request/u,
  );
  assert.equal(seenRequest.requestHash, expectedRequest.requestHash);
  assert.equal(seenRequest.targetDomain, SCCP_DOMAIN_ETH);
  assert.equal(Object.isFrozen(seenRequest), true);

  let acceptedRequest;
  const acceptingSdk = new EthereumMainnetSccp({
    nativeProverArtifacts,
    nativeProverSelfTest(context) {
      return context.expectedResult;
    },
    outboundProver: {
      async prove(request) {
        acceptedRequest = request;
        assert.equal(Object.isFrozen(request.publicSignalWords), true);
        assert.throws(() => {
          request.publicSignalWords[0] = hex32("ff");
        }, TypeError);
        const callbackPublicInputsBytes = request.publicInputsBytes;
        callbackPublicInputsBytes[0] ^= 0x7f;
        assert.notDeepStrictEqual(
          Array.from(callbackPublicInputsBytes),
          Array.from(request.publicInputsBytes),
        );
        const callbackBundleBytes = request.bundleBytes;
        callbackBundleBytes[0] ^= 0x7f;
        assert.deepEqual(
          Array.from(request.bundleBytes),
          Array.from(input.bundleBytes),
        );
        const callbackSourceProofBytes = request.sourceProofBytes;
        assert.deepEqual(Array.from(callbackSourceProofBytes), []);
        return wrapEvmSccpProofResult(groth16ProofBytes(request.publicInputs), request);
      },
    },
  });
  const proofResult = await acceptingSdk.proveOutboundToEthereum(input);
  assert.equal(acceptedRequest.requestHash, expectedRequest.requestHash);
  assert.equal(proofResult.requestHash, expectedRequest.requestHash);
  assert.deepEqual(
    Array.from(proofResult.bundleBytes),
    Array.from(input.bundleBytes),
  );
  assert.deepEqual(proofResult.publicSignalWords, expectedRequest.publicSignalWords);

  const plainSdk = new EthereumMainnetSccp();
  assert.throws(
    () => plainSdk.buildOutboundProofRequest({
      ...input,
      proofArtifactHash: hex32("91"),
    }),
    /proofArtifactHash and provingKeyHash must be supplied together/u,
  );
  assert.throws(
    () => plainSdk.buildOutboundProofRequest({
      ...input,
      proofArtifactHash: hex32("00"),
      provingKeyHash: hex32("92"),
    }),
    /proof request\.proofArtifactHash/u,
  );

  const artifactInput = {
    ...input,
    proofArtifactHash: hex32("91"),
    provingKeyHash: hex32("92"),
  };
  const artifactRequest = plainSdk.buildOutboundProofRequest(artifactInput);
  assert.equal(artifactRequest.proofArtifactHash, hex32("91"));
  assert.equal(artifactRequest.provingKeyHash, hex32("92"));
  assert.notEqual(artifactRequest.requestHash, expectedRequest.requestHash);
  const artifactResult = wrapEvmSccpProofResult(
    groth16ProofBytes(artifactRequest.publicInputs),
    artifactRequest,
  );
  assert.equal(artifactResult.proofArtifactHash, hex32("91"));
  assert.equal(artifactResult.provingKeyHash, hex32("92"));

  const mismatchedArtifactSdk = new EthereumMainnetSccp({
    nativeProverArtifacts,
    nativeProverSelfTest(context) {
      return context.expectedResult;
    },
    outboundProver: {
      async prove() {
        return {
          proofBytes: groth16ProofBytes(input.publicInputs),
          proofArtifactHash: hex32("93"),
          provingKeyHash: hex32("92"),
        };
      },
    },
  });
  await assert.rejects(
    () => mismatchedArtifactSdk.proveOutboundToEthereum(input),
    /proofArtifactHash and provingKeyHash must match request/u,
  );
});

test("EthereumMainnetSccp validates native prover bundle and binds artifact hashes", () => {
  const input = sampleOutboundInput();
  const referenceSdk = new EthereumMainnetSccp();
  const plainRequest = referenceSdk.buildOutboundProofRequest(input);
  const bundle = sampleNativeEvmProverBundle(input.destinationBinding.bindingHash);
  const descriptor = validateEthereumMainnetNativeEvmProverBundle(bundle, {
    destinationBinding: input.destinationBinding,
  });
  const parsedDescriptor = parseEthereumMainnetNativeEvmProverBundleManifest(
    JSON.stringify(bundle),
    { destinationBinding: input.destinationBinding },
  );

  assert.equal(descriptor.schema, SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1);
  assert.equal(descriptor.bundleId, SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1);
  assert.deepEqual(parsedDescriptor, descriptor);
  assert.equal(descriptor.proofArtifactHash, hex32("91"));
  assert.equal(descriptor.proofArtifact, "artifacts/eth-mainnet/proof-artifact.r1cs");
  assert.equal(descriptor.provingKeyHash, hex32("92"));
  assert.equal(descriptor.provingKey, "artifacts/eth-mainnet/proving-key.zkey");
  assert.equal(descriptor.verifierKey, "artifacts/eth-mainnet/verifier-key.bin");
  assert.equal(descriptor.noWasm, true);
  assert.equal(descriptor.remoteProverRequired, false);
  assert.equal(descriptor.browserImplementation, "pure-typescript");
  assert.equal(Object.isFrozen(descriptor), true);
  assert.equal(Object.isFrozen(descriptor.nativeSdkArtifacts), true);
  assert.equal(
    descriptor.nativeSdkArtifacts.find((artifact) => artifact.sdk === "javascript")
      .implementationArtifact,
    "artifacts/eth-mainnet/javascript-implementation.bin",
  );
  assert.throws(() => {
    descriptor.nativeSdkArtifacts[0] = descriptor.nativeSdkArtifacts[1];
  }, TypeError);

  const request = referenceSdk.buildOutboundProofRequest({
    ...input,
    nativeProverBundle: bundle,
  });
  assert.equal(request.proofArtifactHash, descriptor.proofArtifactHash);
  assert.equal(request.provingKeyHash, descriptor.provingKeyHash);
  assert.notEqual(request.requestHash, plainRequest.requestHash);

  const defaultBundleSdk = new EthereumMainnetSccp({
    destinationBinding: input.destinationBinding,
    nativeProverBundle: bundle,
  });
  assert.equal(defaultBundleSdk.buildOutboundProofRequest(input).requestHash, request.requestHash);
  assert.equal(
    referenceSdk.buildOutboundProofRequest({
      ...input,
      native_prover_bundle: bundle,
    }).requestHash,
    request.requestHash,
  );

  assert.throws(
    () =>
      referenceSdk.buildOutboundProofRequest({
        ...input,
        nativeProverBundle: bundle,
        proofArtifactHash: hex32("94"),
        provingKeyHash: hex32("92"),
      }),
    /nativeProverBundle artifact hashes must match proof request/u,
  );
});

test("EthereumMainnetSccp validates native prover cross-SDK parity fixtures", () => {
  const input = sampleOutboundInput();
  const bundle = sampleNativeEvmProverBundle(input.destinationBinding.bindingHash);
  const fixture = sampleNativeEvmProverParityFixture(bundle);
  const descriptor = validateEthereumMainnetNativeEvmProverParityFixture(fixture, bundle);
  const parsedDescriptor = parseEthereumMainnetNativeEvmProverParityFixture(
    JSON.stringify(fixture),
    bundle,
  );

  assert.deepEqual(parsedDescriptor, descriptor);
  assert.equal(descriptor.schema, SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1);
  assert.equal(descriptor.domain, SCCP_DOMAIN_ETH);
  assert.equal(descriptor.chain, "eth");
  assert.equal(descriptor.proofBackend, SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1);
  assert.equal(descriptor.destinationBindingHash, input.destinationBinding.bindingHash);
  assert.equal(descriptor.publicSignalWords.length, 9);
  assert.equal(Object.isFrozen(descriptor), true);
  assert.equal(Object.isFrozen(descriptor.publicSignalWords), true);
  assert.equal(Object.isFrozen(descriptor.sdkResults), true);
  assert.equal(
    descriptor.sdkResults.javascript.toriiSubmitPayloadHash,
    descriptor.toriiSubmitPayloadHash,
  );
  assert.throws(() => {
    descriptor.publicSignalWords[0] = hex32("99");
  }, TypeError);
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverParityFixture(
        sampleNativeEvmProverParityFixture(bundle, {
          destination_binding_hash: hex32("95"),
        }),
        bundle,
      ),
    /destinationBindingHash must match nativeProverBundle/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverParityFixture(
        sampleNativeEvmProverParityFixture(bundle, {
          public_signal_words: descriptor.publicSignalWords.slice(0, 8),
        }),
        bundle,
      ),
    /publicSignalWords must contain 9 words/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverParityFixture(
        sampleNativeEvmProverParityFixture(bundle, {
          sdk_results: {
            ...fixture.sdk_results,
            javascript: {
              ...fixture.sdk_results.javascript,
              calldata_hash: hex32("96"),
            },
          },
        }),
        bundle,
      ),
    /sdkResults\.javascript\.calldataHash must match calldataHash/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverParityFixture(
        sampleNativeEvmProverParityFixture(bundle, {
          sdk_results: {
            ...fixture.sdk_results,
            browser: fixture.sdk_results.javascript,
          },
        }),
        bundle,
      ),
    /sdkResults contains unknown sdk: browser/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverParityFixture(
        {
          ...fixture,
          proofArtifactHash: fixture.proof_artifact_hash,
        },
        bundle,
      ),
    /proofArtifactHash must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      parseEthereumMainnetNativeEvmProverParityFixture(
        JSON.stringify(fixture).replace(
          `"schema":"${SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1}"`,
          `"schema":"forged","schema":"${SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1}"`,
        ),
        bundle,
      ),
    /nativeProverParityFixture contains duplicate JSON key: schema/u,
  );
});

test("EthereumMainnetSccp validates native prover self-test fixtures", () => {
  const input = sampleOutboundInput();
  const bundle = sampleNativeEvmProverBundle(input.destinationBinding.bindingHash);
  const fixture = sampleNativeEvmProverSelfTestFixture(bundle);
  const descriptor = validateEthereumMainnetNativeEvmProverSelfTestFixture(fixture, bundle);
  const parsedDescriptor = parseEthereumMainnetNativeEvmProverSelfTestFixture(
    JSON.stringify(fixture),
    bundle,
  );

  assert.deepEqual(parsedDescriptor, descriptor);
  assert.equal(descriptor.schema, SCCP_ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1);
  assert.equal(descriptor.destinationBindingHash, input.destinationBinding.bindingHash);
  assert.equal(descriptor.publicSignalWords.length, 9);
  assert.equal(descriptor.sdkResults.javascript.proofHash, descriptor.proofHash);
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverSelfTestFixture(
        sampleNativeEvmProverSelfTestFixture(bundle, {
          sdk_results: {
            ...fixture.sdk_results,
            javascript: {
              ...fixture.sdk_results.javascript,
              proof_hash: hex32("97"),
            },
          },
        }),
        bundle,
      ),
    /sdkResults\.javascript\.proofHash must match proofHash/u,
  );
  assert.throws(
    () =>
      parseEthereumMainnetNativeEvmProverSelfTestFixture(
        JSON.stringify(fixture).replace(
          `"schema":"${SCCP_ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1}"`,
          `"schema":"forged","schema":"${SCCP_ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1}"`,
        ),
        bundle,
      ),
    /nativeProverSelfTestFixture contains duplicate JSON key: schema/u,
  );
});

test("EthereumMainnetSccp verifies native prover artifact bytes against manifest hashes", async () => {
  const proofArtifactBytes = nativeEvmSnarkjsArtifactBytes(
    "sccp proof artifact v1",
    "r1cs",
    3,
  );
  const provingKeyBytes = nativeEvmSnarkjsArtifactBytes(
    "sccp proving key v1",
    "zkey",
    10,
  );
  const verifierKeyBytes = nativeEvmProverArtifactBytes("sccp verifier key v1");
  const implementationBytes = nativeEvmProverArtifactBytes(
    "sccp pure typescript prover artifact v1",
  );
  const proofArtifactHash = sha256Hex(proofArtifactBytes);
  const provingKeyHash = sha256Hex(provingKeyBytes);
  const verifierKeyHash = sha256Hex(verifierKeyBytes);
  const implementationHash = sha256Hex(implementationBytes);
  const input = sampleOutboundInput(SCCP_DOMAIN_ETH, { verifierKeyHash });
  const hashConsistentNativeEvmProverBundle = ({
    proofArtifactBytes: selectedProofArtifactBytes = proofArtifactBytes,
    provingKeyBytes: selectedProvingKeyBytes = provingKeyBytes,
    verifierKeyBytes: selectedVerifierKeyBytes = verifierKeyBytes,
    implementationBytes: selectedImplementationBytes = implementationBytes,
    crossSdkFixtureParityBytes: selectedParityFixtureBytes,
    nativeProverSelfTestBytes: selectedSelfTestFixtureBytes,
  } = {}) => {
    const selectedProofArtifactHash = sha256Hex(selectedProofArtifactBytes);
    const selectedProvingKeyHash = sha256Hex(selectedProvingKeyBytes);
    const selectedVerifierKeyHash = sha256Hex(selectedVerifierKeyBytes);
    const selectedImplementationHash = sha256Hex(selectedImplementationBytes);
    const draftBundle = sampleNativeEvmProverBundle(
      input.destinationBinding.bindingHash,
      {
        proof_artifact_hash: selectedProofArtifactHash,
        proving_key_hash: selectedProvingKeyHash,
        verifier_key_hash: selectedVerifierKeyHash,
        native_sdk_artifacts: Object.entries(
          SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
        ).map(([sdk, implementation], index) => ({
          sdk,
          implementation,
          prover_artifact_hash: selectedProofArtifactHash,
          proving_key_hash: selectedProvingKeyHash,
          implementation_artifact: `artifacts/eth-mainnet/${sdk}-implementation.bin`,
          implementation_hash: sdk === "javascript"
            ? selectedImplementationHash
            : hex32((index + 1).toString(16).padStart(2, "0")),
        })),
      },
    );
    const parityFixtureBytesForBundle =
      selectedParityFixtureBytes ?? sampleNativeEvmProverParityFixtureBytes(draftBundle);
    const selfTestFixtureBytesForBundle =
      selectedSelfTestFixtureBytes ?? sampleNativeEvmProverSelfTestFixtureBytes(draftBundle);
    return {
      bundle: sampleNativeEvmProverBundle(input.destinationBinding.bindingHash, {
        proof_artifact_hash: selectedProofArtifactHash,
        proving_key_hash: selectedProvingKeyHash,
        verifier_key_hash: selectedVerifierKeyHash,
        native_sdk_artifacts: draftBundle.native_sdk_artifacts,
        audit_hashes: {
          ...draftBundle.audit_hashes,
          cross_sdk_fixture_parity: sha256Hex(parityFixtureBytesForBundle),
          native_prover_self_test: sha256Hex(selfTestFixtureBytesForBundle),
        },
      }),
      parityFixtureBytes: parityFixtureBytesForBundle,
      selfTestFixtureBytes: selfTestFixtureBytesForBundle,
    };
  };
  const { bundle, parityFixtureBytes, parityFixtureHash, selfTestFixtureBytes, selfTestFixtureHash } =
    sampleNativeEvmProverBundleWithFixtureBytes(input.destinationBinding.bindingHash, {
    proof_artifact_hash: proofArtifactHash,
    proving_key_hash: provingKeyHash,
    verifier_key_hash: verifierKeyHash,
    native_sdk_artifacts: Object.entries(
      SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
    ).map(([sdk, implementation], index) => ({
      sdk,
      implementation,
      prover_artifact_hash: proofArtifactHash,
      proving_key_hash: provingKeyHash,
      implementation_artifact: `artifacts/eth-mainnet/${sdk}-implementation.bin`,
      implementation_hash: sdk === "javascript"
        ? implementationHash
        : hex32((index + 1).toString(16).padStart(2, "0")),
    })),
  });

  const verified = verifyEthereumMainnetNativeEvmProverArtifacts(
    {
      nativeProverBundle: JSON.stringify(bundle),
      proofArtifactBytes,
      provingKeyBytes,
      verifierKeyBytes,
      crossSdkFixtureParityBytes: parityFixtureBytes,
      nativeProverSelfTestBytes: selfTestFixtureBytes,
      sdk: "javascript",
      implementationBytes,
    },
    { destinationBinding: input.destinationBinding },
  );

  assert.equal(verified.hashAlgorithm, SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1);
  assert.equal(verified.proofArtifactHash, proofArtifactHash);
  assert.equal(verified.provingKeyHash, provingKeyHash);
  assert.equal(verified.verifierKeyHash, verifierKeyHash);
  assert.equal(verified.crossSdkFixtureParityHash, parityFixtureHash);
  assert.equal(verified.crossSdkFixtureParity.sdkResults.javascript.calldataHash, hex32("d3"));
  assert.equal(verified.nativeProverSelfTestHash, selfTestFixtureHash);
  assert.equal(verified.nativeProverSelfTest.sdkResults.javascript.proofHash, hex32("e4"));
  assert.equal(verified.nativeProverBundle.proofArtifactHash, proofArtifactHash);
  assert.equal(verified.sdk, "javascript");
  assert.equal(verified.implementation, "pure-typescript");
  assert.equal(verified.implementationHash, implementationHash);
  assert.equal(Object.isFrozen(verified), true);
  const artifactBytesByPath = new Map([
    [verified.nativeProverBundle.proofArtifact, proofArtifactBytes],
    [verified.nativeProverBundle.provingKey, provingKeyBytes],
    [verified.nativeProverBundle.verifierKey, verifierKeyBytes],
    [verified.nativeProverBundle.crossSdkFixtureParityArtifact, parityFixtureBytes],
    [verified.nativeProverBundle.nativeProverSelfTestArtifact, selfTestFixtureBytes],
    [
      verified.nativeProverBundle.nativeSdkArtifacts.find((row) => row.sdk === "javascript")
        .implementationArtifact,
      implementationBytes,
    ],
  ]);
  const resolvedArtifacts = [];
  const verifiedFromBundle = await verifyEthereumMainnetNativeEvmProverArtifactsFromBundle(
    {
      nativeProverBundle: bundle,
      sdk: "javascript",
      artifactResolver(path, metadata) {
        resolvedArtifacts.push(`${metadata.role}:${path}`);
        return artifactBytesByPath.get(path);
      },
    },
    { destinationBinding: input.destinationBinding },
  );
  assert.equal(verifiedFromBundle.implementationHash, implementationHash);
  assert.equal(verifiedFromBundle.crossSdkFixtureParityHash, parityFixtureHash);
  assert.equal(verifiedFromBundle.nativeProverSelfTestHash, selfTestFixtureHash);
  await assert.rejects(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifactsFromBundle(
        {
          nativeProverBundle: bundle,
          sdk: " javascript ",
          artifactResolver(path) {
            return artifactBytesByPath.get(path);
          },
        },
        { destinationBinding: input.destinationBinding },
      ),
    /nativeProverArtifacts\.sdk must be a non-empty canonical string/u,
  );
  let helperSawOptions = false;
  const helperSelfTestResult = await runEthereumMainnetNativeProverSelfTest(
    {
      nativeProverArtifacts: verified,
      nativeProverSelfTest(context, options) {
        helperSawOptions = options?.preflight === true;
        assert.equal(Object.isFrozen(context.nativeProverArtifacts), true);
        assert.equal(context.nativeProverSelfTest.proofHash, hex32("e4"));
        return context.expectedResult;
      },
    },
    { preflight: true },
  );
  assert.equal(helperSawOptions, true);
  assert.equal(helperSelfTestResult.proofHash, hex32("e4"));
  let paddedSelfTestHookCalled = false;
  await assert.rejects(
    () =>
      runEthereumMainnetNativeProverSelfTest({
        nativeProverArtifacts: { ...verified, sdk: " javascript " },
        nativeProverSelfTest() {
          paddedSelfTestHookCalled = true;
          return verified.nativeProverSelfTest.sdkResults.javascript;
        },
      }),
    /nativeProverArtifacts must be returned by the local native EVM prover artifact byte verifier/u,
  );
  assert.equal(paddedSelfTestHookCalled, false);
  assert.deepEqual(resolvedArtifacts, [
    `proofArtifact:${verified.nativeProverBundle.proofArtifact}`,
    `provingKey:${verified.nativeProverBundle.provingKey}`,
    `verifierKey:${verified.nativeProverBundle.verifierKey}`,
    `crossSdkFixtureParityArtifact:${verified.nativeProverBundle.crossSdkFixtureParityArtifact}`,
    `nativeProverSelfTestArtifact:${verified.nativeProverBundle.nativeProverSelfTestArtifact}`,
    `implementationArtifact:${
      verified.nativeProverBundle.nativeSdkArtifacts.find((row) => row.sdk === "javascript")
        .implementationArtifact
    }`,
  ]);
  let factoryBoundRequest;
  const sdkFromBundle = await EthereumMainnetSccp.fromNativeProverBundle({
    destinationBinding: input.destinationBinding,
    manifest: JSON.stringify(bundle),
    sdk: "javascript",
    artifactResolver(path, metadata) {
      assert.equal(Object.isFrozen(metadata), true);
      return artifactBytesByPath.get(path);
    },
    async nativeProverSelfTest(context) {
      assert.equal(Object.isFrozen(context), true);
      assert.equal(context.sdk, "javascript");
      assert.equal(context.nativeProverSelfTest.proofHash, hex32("e4"));
      return context.expectedResult;
    },
    outboundProver: {
      async prove(request) {
        factoryBoundRequest = request;
        return wrapEvmSccpProofResult(groth16ProofBytes(request.publicInputs), request);
      },
    },
  });
  const factoryProofResult = await sdkFromBundle.proveOutboundToEthereum(input);
  assert.equal(factoryBoundRequest.proofArtifactHash, proofArtifactHash);
  assert.equal(factoryBoundRequest.provingKeyHash, provingKeyHash);
  assert.equal(factoryProofResult.destinationBinding.verifierKeyHash, verifierKeyHash);
  assert.equal(
    sdkFromBundle.buildEthereumCalldata({ proofResult: factoryProofResult }).destinationBindingHash,
    input.destinationBinding.bindingHash,
  );
  await assert.rejects(
    () =>
      EthereumMainnetSccp.fromNativeProverBundle({
        destinationBinding: input.destinationBinding,
        nativeProverBundle: bundle,
        nativeProverArtifacts: verified,
        sdk: "javascript",
        artifactResolver(path) {
          return artifactBytesByPath.get(path);
        },
      }),
    /pass nativeProverArtifacts to the constructor directly/u,
  );
  await assert.rejects(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifactsFromBundle(
        {
          nativeProverBundle: bundle,
          sdk: "javascript",
          artifactResolver(path) {
            return path === verified.nativeProverBundle.crossSdkFixtureParityArtifact
              ? undefined
              : artifactBytesByPath.get(path);
          },
        },
        { destinationBinding: input.destinationBinding },
      ),
    /crossSdkFixtureParityBytes resolver returned no bytes/u,
  );
  let preflightHookCalls = 0;
  const preflightSdk = new EthereumMainnetSccp({
    destinationBinding: input.destinationBinding,
    nativeProverArtifacts: verified,
    nativeProverSelfTest(context) {
      preflightHookCalls += 1;
      return context.expectedResult;
    },
    outboundProver: {
      async prove() {
        throw new Error("preflight must not call prove");
      },
    },
  });
  assert.equal((await preflightSdk.runNativeProverSelfTest()).calldataHash, hex32("e5"));
  assert.equal(preflightHookCalls, 1);
  await assert.rejects(
    () => new EthereumMainnetSccp().runNativeProverSelfTest(),
    /verified native EVM prover artifacts/u,
  );
  const unverifiedDescriptorMessage =
    /nativeProverArtifacts must be returned by the local native EVM prover artifact byte verifier/u;
  let forgedDescriptorSelfTestCalled = false;
  assert.throws(
    () =>
      new EthereumMainnetSccp({
        destinationBinding: input.destinationBinding,
        nativeProverArtifacts: { ...verified },
        nativeProverSelfTest() {
          forgedDescriptorSelfTestCalled = true;
          return verified.nativeProverSelfTest.sdkResults.javascript;
        },
      }),
    unverifiedDescriptorMessage,
  );
  assert.equal(forgedDescriptorSelfTestCalled, false);
  const { implementationHash: _implementationHash, ...missingImplementationHash } = verified;
  assert.throws(
    () =>
      new EthereumMainnetSccp({
        destinationBinding: input.destinationBinding,
        nativeProverArtifacts: missingImplementationHash,
      }),
    unverifiedDescriptorMessage,
  );
  assert.throws(
    () =>
      new EthereumMainnetSccp({
        destinationBinding: input.destinationBinding,
        nativeProverArtifacts: {
          ...verified,
          sdk: " javascript ",
        },
      }),
    unverifiedDescriptorMessage,
  );
  assert.throws(
    () =>
      new EthereumMainnetSccp({
        destinationBinding: input.destinationBinding,
        nativeProverArtifacts: {
          ...verified,
          verifierKeyHash: hex32("ef"),
        },
      }),
    unverifiedDescriptorMessage,
  );
  let artifactBoundRequest;
  const artifactBoundSdk = new EthereumMainnetSccp({
    destinationBinding: input.destinationBinding,
    nativeProverBundle: bundle,
    proofArtifactBytes,
    provingKeyBytes,
    verifierKeyBytes,
    crossSdkFixtureParityBytes: parityFixtureBytes,
    nativeProverSelfTestBytes: selfTestFixtureBytes,
    sdk: "javascript",
    implementationBytes,
    nativeProverSelfTest(context) {
      assert.equal(context.nativeProverArtifacts.nativeProverSelfTestHash, selfTestFixtureHash);
      return context.expectedResult;
    },
    outboundProver: {
      async prove(request) {
        artifactBoundRequest = request;
        return wrapEvmSccpProofResult(groth16ProofBytes(request.publicInputs), request);
      },
    },
  });
  const proofResult = await artifactBoundSdk.proveOutboundToEthereum(input);
  assert.equal(artifactBoundRequest.proofArtifactHash, proofArtifactHash);
  assert.equal(artifactBoundRequest.provingKeyHash, provingKeyHash);
  assert.equal(proofResult.requestHash, artifactBoundRequest.requestHash);

  let missingSelfTestProverCalled = false;
  const missingSelfTestSdk = new EthereumMainnetSccp({
    destinationBinding: input.destinationBinding,
    nativeProverArtifacts: verified,
    outboundProver: {
      async prove(request) {
        missingSelfTestProverCalled = true;
        return wrapEvmSccpProofResult(groth16ProofBytes(request.publicInputs), request);
      },
    },
  });
  await assert.rejects(
    () => missingSelfTestSdk.proveOutboundToEthereum(input),
    /native prover self-test hook/u,
  );
  assert.equal(missingSelfTestProverCalled, false);

  let tamperedSelfTestProverCalled = false;
  const tamperedSelfTestSdk = new EthereumMainnetSccp({
    destinationBinding: input.destinationBinding,
    nativeProverArtifacts: verified,
    nativeProverSelfTest(context) {
      return { ...context.expectedResult, proofHash: hex32("97") };
    },
    outboundProver: {
      async prove(request) {
        tamperedSelfTestProverCalled = true;
        return wrapEvmSccpProofResult(groth16ProofBytes(request.publicInputs), request);
      },
    },
  });
  await assert.rejects(
    () => tamperedSelfTestSdk.proveOutboundToEthereum(input),
    /sdkResults.javascript.proofHash must match proofHash/u,
  );
  assert.equal(tamperedSelfTestProverCalled, false);

  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes: Uint8Array.from([0]),
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: parityFixtureBytes,
          nativeProverSelfTestBytes: selfTestFixtureBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /proofArtifactBytes sha256/u,
  );
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes,
          provingKeyBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /verifierKeyBytes/u,
  );
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: parityFixtureBytes,
          nativeProverSelfTestBytes: selfTestFixtureBytes,
          implementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /nativeProverArtifacts\.sdk must be a non-empty canonical string/u,
  );
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: parityFixtureBytes,
          nativeProverSelfTestBytes: selfTestFixtureBytes,
          sdk: " javascript ",
          implementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /nativeProverArtifacts\.sdk must be a non-empty canonical string/u,
  );
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: parityFixtureBytes,
          nativeProverSelfTestBytes: selfTestFixtureBytes,
          sdk: "javascript",
        },
        { destinationBinding: input.destinationBinding },
      ),
    /implementationBytes are required/u,
  );
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: parityFixtureBytes,
          nativeProverSelfTestBytes: selfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes: Buffer.from("tampered", "utf8"),
        },
        { destinationBinding: input.destinationBinding },
      ),
    /implementationBytes sha256/u,
  );
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          sdk: "javascript",
          implementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /crossSdkFixtureParityBytes is required/u,
  );
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: parityFixtureBytes,
          sdk: "javascript",
          implementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /nativeProverSelfTestBytes is required/u,
  );
  const tinyProofArtifactBytes = Buffer.from("tiny native proof artifact\n", "utf8");
  const tinyProofArtifactHash = sha256Hex(tinyProofArtifactBytes);
  const {
    bundle: tinyBundle,
    parityFixtureBytes: tinyParityFixtureBytes,
    selfTestFixtureBytes: tinySelfTestFixtureBytes,
  } =
    sampleNativeEvmProverBundleWithFixtureBytes(input.destinationBinding.bindingHash, {
    proof_artifact_hash: tinyProofArtifactHash,
    proving_key_hash: provingKeyHash,
    verifier_key_hash: verifierKeyHash,
    native_sdk_artifacts: bundle.native_sdk_artifacts.map((artifact) => ({
      ...artifact,
      prover_artifact_hash: tinyProofArtifactHash,
    })),
  });
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: tinyBundle,
          proofArtifactBytes: tinyProofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: tinyParityFixtureBytes,
          nativeProverSelfTestBytes: tinySelfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /proofArtifactBytes must be at least 65536 bytes/u,
  );
  const tinyProvingKeyBytes = Buffer.from("tiny native proving key\n", "utf8");
  const tinyProvingKeyBundle = hashConsistentNativeEvmProverBundle({
    provingKeyBytes: tinyProvingKeyBytes,
  });
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: tinyProvingKeyBundle.bundle,
          proofArtifactBytes,
          provingKeyBytes: tinyProvingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: tinyProvingKeyBundle.parityFixtureBytes,
          nativeProverSelfTestBytes: tinyProvingKeyBundle.selfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /provingKeyBytes must be at least 65536 bytes/u,
  );
  const tinyVerifierKeyBytes = Buffer.from("tiny native verifier key\n", "utf8");
  const tinyVerifierKeyBundle = hashConsistentNativeEvmProverBundle({
    verifierKeyBytes: tinyVerifierKeyBytes,
  });
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: tinyVerifierKeyBundle.bundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes: tinyVerifierKeyBytes,
          crossSdkFixtureParityBytes: tinyVerifierKeyBundle.parityFixtureBytes,
          nativeProverSelfTestBytes: tinyVerifierKeyBundle.selfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /verifierKeyBytes must be at least 128 bytes/u,
  );
  const tinyParitySupportFixtureBytes = Buffer.from("{}", "utf8");
  const tinyParitySupportBundle = hashConsistentNativeEvmProverBundle({
    crossSdkFixtureParityBytes: tinyParitySupportFixtureBytes,
  });
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: tinyParitySupportBundle.bundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: tinyParitySupportFixtureBytes,
          nativeProverSelfTestBytes: tinyParitySupportBundle.selfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /crossSdkFixtureParityBytes must be at least 128 bytes/u,
  );
  const tinySelfTestSupportFixtureBytes = Buffer.from("{}", "utf8");
  const tinySelfTestSupportBundle = hashConsistentNativeEvmProverBundle({
    nativeProverSelfTestBytes: tinySelfTestSupportFixtureBytes,
  });
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: tinySelfTestSupportBundle.bundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: tinySelfTestSupportBundle.parityFixtureBytes,
          nativeProverSelfTestBytes: tinySelfTestSupportFixtureBytes,
          sdk: "javascript",
          implementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /nativeProverSelfTestBytes must be at least 128 bytes/u,
  );
  const tinyImplementationBytes = Buffer.from("tiny native js implementation\n", "utf8");
  const tinyImplementationBundle = hashConsistentNativeEvmProverBundle({
    implementationBytes: tinyImplementationBytes,
  });
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: tinyImplementationBundle.bundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: tinyImplementationBundle.parityFixtureBytes,
          nativeProverSelfTestBytes: tinyImplementationBundle.selfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes: tinyImplementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /implementationBytes must be at least 1024 bytes/u,
  );
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: Buffer.from("{}", "utf8"),
          nativeProverSelfTestBytes: selfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /crossSdkFixtureParityBytes sha256/u,
  );
  const flaggedArtifactBytes = nativeEvmSnarkjsArtifactBytes(
    "native proof artifact imports local prover code",
    "r1cs",
    3,
  );
  flaggedArtifactBytes.set(Buffer.from("proof.wasm", "utf8"), 1024);
  const flaggedArtifactHash = sha256Hex(flaggedArtifactBytes);
  const {
    bundle: flaggedBundle,
    parityFixtureBytes: flaggedParityFixtureBytes,
    selfTestFixtureBytes: flaggedSelfTestFixtureBytes,
  } =
    sampleNativeEvmProverBundleWithFixtureBytes(input.destinationBinding.bindingHash, {
    proof_artifact_hash: flaggedArtifactHash,
    proving_key_hash: provingKeyHash,
    verifier_key_hash: verifierKeyHash,
    native_sdk_artifacts: bundle.native_sdk_artifacts.map((artifact) => ({
      ...artifact,
      prover_artifact_hash: flaggedArtifactHash,
    })),
  });
  assert.throws(
    () =>
      verifyEthereumMainnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: flaggedBundle,
          proofArtifactBytes: flaggedArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          crossSdkFixtureParityBytes: flaggedParityFixtureBytes,
          nativeProverSelfTestBytes: flaggedSelfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes,
        },
        { destinationBinding: input.destinationBinding },
      ),
    /proofArtifactBytes contains forbidden prover dependency marker/u,
  );
});

test("EthereumMainnetSccp rejects unsafe native EVM prover bundle manifests", () => {
  const input = sampleOutboundInput();
  const bundle = sampleNativeEvmProverBundle(input.destinationBinding.bindingHash);

  assert.throws(
    () =>
      parseEthereumMainnetNativeEvmProverBundleManifest(
        JSON.stringify(bundle).replace(
          `"bundle_id":"${SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1}"`,
          `"bundle_id":"forged","bundle_id":"${SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1}"`,
        ),
      ),
    /nativeProverBundle contains duplicate JSON key: bundle_id/u,
  );
  assert.throws(
    () =>
      parseEthereumMainnetNativeEvmProverBundleManifest(
        JSON.stringify(bundle).replace(
          `"bundle_id":"${SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1}"`,
          `"bundle\\u005fid":"forged","bundle_id":"${SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1}"`,
        ),
      ),
    /nativeProverBundle contains duplicate JSON key: bundle_id/u,
  );
  assert.throws(
    () => validateEthereumMainnetNativeEvmProverBundle({ ...bundle, no_wasm: false }),
    /noWasm must be true/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        remote_prover_required: true,
      }),
    /remoteProverRequired must be false/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        domain: SCCP_DOMAIN_BSC,
      }),
    /domain must be Ethereum mainnet/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        domain: "01",
      }),
    /domain must be a u32 domain id/u,
  );
  assert.throws(
    () => validateEthereumMainnetNativeEvmProverBundle({ ...bundle, chain: "bsc" }),
    /chain must be eth/u,
  );
  assert.throws(
    () =>
      new EthereumMainnetSccp().buildOutboundProofRequest({
        ...input,
        nativeProverBundle: {
          ...bundle,
          destination_binding_hash: hex32("95"),
        },
      }),
    /nativeProverBundle destinationBindingHash must match destinationBinding/u,
  );
  assert.throws(
    () =>
      new EthereumMainnetSccp().buildOutboundProofRequest({
        ...input,
        nativeProverBundle: {
          ...bundle,
          verifier_key_hash: hex32("dd"),
        },
      }),
    /nativeProverBundle verifierKeyHash must match destinationBinding/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        proof_artifact: "../proof-artifact.r1cs",
      }),
    /proofArtifact must stay under the manifest directory/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        proof_artifact: "artifacts/ethereum-mainnet/fixtures/proof-artifact.r1cs",
      }),
    /proofArtifact must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        proof_artifact: "artifacts/eth-mainnet/proof-artifact.bin",
      }),
    /proofArtifact must reference a \.r1cs artifact/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        proving_key: "artifacts/eth-mainnet/proving-key.bin",
      }),
    /provingKey must reference a \.zkey artifact/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        verifier_key: "/tmp/verifier-key.bin",
      }),
    /verifierKey must be a relative POSIX path/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        audit_hashes: [hex32("a1")],
      }),
    /auditHashes must be an object/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        audit_hashes: {
          ...bundle.audit_hashes,
          circuit_security_audit: hex32("a1").toUpperCase(),
        },
      }),
    /auditHashes\.circuit_security_audit must be canonical lowercase 0x-prefixed 32-byte hex/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        audit_hashes: {
          ...bundle.audit_hashes,
          circuit_security_audit: bundle.proof_artifact_hash,
        },
      }),
    /nativeProverBundle hashes must be role-separated: auditHashes\.circuit_security_audit matches proofArtifactHash/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        audit_hashes: {
          ...bundle.audit_hashes,
          native_implementation_audit: hex32("a2"),
        },
      }),
    /auditHashes\.native_implementation_audit must not look like a placeholder audit hash: repeated 1-byte pattern/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        audit_hashes: {
          ...bundle.audit_hashes,
          reproducible_build_attestation: `0x${Array.from(
            { length: 32 },
            (_, index) => index.toString(16).padStart(2, "0"),
          ).join("")}`,
        },
      }),
    /auditHashes\.reproducible_build_attestation must not look like a placeholder audit hash: arithmetic byte sequence/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        experimental_manifest_note: "ignored fields must fail",
      }),
    /nativeProverBundle contains unknown field: experimental_manifest_note/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        proofArtifactHash: bundle.proof_artifact_hash,
      }),
    /proofArtifactHash must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        native_sdk_artifacts: bundle.native_sdk_artifacts.map((artifact) =>
          artifact.sdk === "javascript"
            ? { ...artifact, sdk: " javascript " }
            : artifact,
        ),
      }),
    /nativeSdkArtifacts\[\d+\]\.sdk must be a non-empty canonical string/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        native_sdk_artifacts: bundle.native_sdk_artifacts.map((artifact) =>
          artifact.sdk === "javascript"
            ? { ...artifact, experimental_manifest_note: "ignored fields must fail" }
            : artifact,
        ),
      }),
    /nativeSdkArtifacts\[0\] contains unknown field: experimental_manifest_note/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        native_sdk_artifacts: bundle.native_sdk_artifacts.map((artifact) =>
          artifact.sdk === "javascript"
            ? { ...artifact, implementation_artifact: "native\\javascript.bin" }
            : artifact,
        ),
      }),
    /implementationArtifact must be a relative POSIX path/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        native_sdk_artifacts: bundle.native_sdk_artifacts.map((artifact) =>
          artifact.sdk === "javascript"
            ? {
                ...artifact,
                implementation_artifact:
                  "artifacts/ethereum-mainnet/mock/javascript.bin",
              }
            : artifact,
        ),
      }),
    /nativeSdkArtifacts\[0\]\.implementationArtifact must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        native_sdk_artifacts: bundle.native_sdk_artifacts.filter(
          (artifact) => artifact.sdk !== "swift",
        ),
      }),
    /nativeSdkArtifacts missing sdk: swift/u,
  );
  assert.throws(
    () =>
      validateEthereumMainnetNativeEvmProverBundle({
        ...bundle,
        native_sdk_artifacts: bundle.native_sdk_artifacts.map((artifact) =>
          artifact.sdk === "javascript"
            ? { ...artifact, implementation: "wasm-witness" }
            : artifact,
        ),
      }),
    /javascript implementation must be pure-typescript/u,
  );
});

test("EthereumMainnetSccp outbound provider path derives target from wrapped proof result", async () => {
  const submittedTxs = [];
  const provider = {
    async request({ method, params }) {
      if (method === "eth_chainId") return "0x1";
      if (method === "eth_sendTransaction") {
        submittedTxs.push(params[0]);
        return `0xeth${submittedTxs.length}`;
      }
      throw new Error(`unexpected RPC method ${method}`);
    },
  };
  const { destinationBinding, nativeProverArtifacts } = sampleVerifiedNativeEvmProverFixture();
  const sdk = new EthereumMainnetSccp({ executionProvider: provider, nativeProverArtifacts });
  const request = sdk.buildOutboundProofRequest({
    ...sampleOutboundInput(),
    destinationBinding,
  });
  const proofResult = wrapEvmSccpProofResult(
    groth16ProofBytes(request.publicInputs),
    request,
  );

  assert.equal(await sdk.submitOutboundToEthereum({ proofResult }), "0xeth1");
  assert.equal(submittedTxs[0].to, request.destinationBinding.bridgeAddress);
  assert.equal(submittedTxs[0].data, sdk.buildEthereumCalldata({ proofResult }).callDataHex);
  assert.equal(submittedTxs[0].chainId, "0x1");

  const { destinationBinding: proofResultBinding, ...proofResultWithoutBinding } = proofResult;
  const { bridgeAddress: _bridgeAddress, ...bindingWithoutBridge } = proofResultBinding;
  const snakeProofResult = {
    ...proofResultWithoutBinding,
    destination_binding: {
      ...bindingWithoutBridge,
      bridge_address: proofResultBinding.bridgeAddress,
    },
  };
  assert.equal(await sdk.submitOutboundToEthereum({ proof_result: snakeProofResult }), "0xeth2");
  assert.equal(submittedTxs[1].to, request.destinationBinding.bridgeAddress);
  assert.equal(submittedTxs[1].chainId, "0x1");

  assert.equal(
    await sdk.submitOutboundToEthereum({
      proofResult,
      to: request.destinationBinding.bridgeAddress.toUpperCase(),
    }),
    "0xeth3",
  );
  assert.equal(submittedTxs[2].to, request.destinationBinding.bridgeAddress);
  assert.equal(submittedTxs[2].chainId, "0x1");

  assert.equal(
    await sdk.submitOutboundToEthereum({
      proofResult,
      from: `0x${"AA".repeat(20)}`,
    }),
    "0xeth4",
  );
  assert.equal(submittedTxs[3].from, `0x${"aa".repeat(20)}`);
  assert.equal(submittedTxs[3].chainId, "0x1");

  await assert.rejects(
    () => sdk.submitOutboundToEthereum({ proofResult, from: `0x${"00".repeat(20)}` }),
    /Ethereum mainnet SCCP outbound from must not be zero/u,
  );
  assert.equal(submittedTxs.length, 4);

  await assert.rejects(
    () => sdk.submitOutboundToEthereum({ proofResult, to: `0x${"77".repeat(20)}` }),
    /to address must match proofResult\.destinationBinding\.bridgeAddress/u,
  );
  assert.equal(submittedTxs.length, 4);

  let guardedSubmitterCalled = false;
  const guardedSdk = new EthereumMainnetSccp({
    nativeProverArtifacts,
    executionProvider: {
      async request({ method }) {
        assert.equal(method, "eth_chainId");
        return "0x38";
      },
    },
    submitOutboundToEthereum() {
      guardedSubmitterCalled = true;
      return "wrong-chain";
    },
  });
  await assert.rejects(
    () => guardedSdk.submitOutboundToEthereum({ proofResult }),
    /eth_chainId == 0x1/u,
  );
  assert.equal(guardedSubmitterCalled, false);
});

test("EthereumMainnetSccp builds ETH -> SORA local-admission submissions", () => {
  const input = {
    sourceDomain: SCCP_DOMAIN_ETH,
    targetDomain: SCCP_DOMAIN_SORA,
    proofBytes: [1, 2, 3],
    publicInputsBytes: [4, 5, 6],
    bundleBytes: [7, 8, 9],
    envelopeBytes: [10, 11, 12],
    statementHash: hex32("66"),
    sourceVerifierMaterialHash: hex32("77"),
    sourceAdapterEngineDeploymentHash: hex32("88"),
  };
  const submission = buildEthereumMainnetSccpLocalAdmissionSubmission(input);
  const facadeSubmission = new EthereumMainnetSccp().buildLocalAdmissionSubmission(input);

  assert.equal(submission.platformPayload, SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1);
  assert.equal(submission.envelopeEncoding, SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1);
  assert.equal(submission.verifierEntrypoint, SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1);
  assert.equal(submission.sourceDomain, SCCP_DOMAIN_ETH);
  assert.equal(submission.targetDomain, SCCP_DOMAIN_SORA);
  assert.deepEqual([...submission.arguments], []);
  assert.deepEqual([...submission.proofBytes], [1, 2, 3]);
  assert.deepEqual([...submission.publicInputsBytes], [4, 5, 6]);
  assert.deepEqual([...submission.bundleBytes], [7, 8, 9]);
  assert.deepEqual([...submission.envelopeBytes], [10, 11, 12]);
  assert.deepEqual([...submission.localAdmission.proofBytes], [1, 2, 3]);
  assert.equal(facadeSubmission.envelopeHex, submission.envelopeHex);

  input.proofBytes[0] = 99;
  assert.deepEqual([...submission.proofBytes], [1, 2, 3]);

  assert.throws(
    () => buildEthereumMainnetSccpLocalAdmissionSubmission({ ...input, sourceDomain: SCCP_DOMAIN_BSC }),
    /ETH -> SORA/u,
  );
  assert.throws(
    () => buildEthereumMainnetSccpLocalAdmissionSubmission({ ...input, proofBytes: [0, 0] }),
    /proofBytes must not be all zero/u,
  );
  assert.throws(
    () => buildEthereumMainnetSccpLocalAdmissionSubmission({ ...input, publicInputsBytes: [0, 0] }),
    /publicInputsBytes must not be all zero/u,
  );
  assert.throws(
    () => buildEthereumMainnetSccpLocalAdmissionSubmission({ ...input, bundleBytes: [0, 0] }),
    /bundleBytes must not be all zero/u,
  );
  assert.throws(
    () => buildEthereumMainnetSccpLocalAdmissionSubmission({ ...input, envelopeBytes: [] }),
    /envelopeBytes must not be empty/u,
  );
  assert.throws(
    () => buildEthereumMainnetSccpLocalAdmissionSubmission({ ...input, envelopeBytes: [0, 0] }),
    /envelopeBytes must not be all zero/u,
  );
  assert.throws(
    () => buildEthereumMainnetSccpLocalAdmissionSubmission({ ...input, statementHash: hex32("00") }),
    /statementHash must not be zero/u,
  );
  assert.throws(
    () => buildEthereumMainnetSccpLocalAdmissionSubmission({ ...input, sourceVerifierMaterialHash: hex32("00") }),
    /sourceVerifierMaterialHash must not be zero/u,
  );
  assert.throws(
    () => buildEthereumMainnetSccpLocalAdmissionSubmission({ ...input, sourceAdapterEngineDeploymentHash: hex32("00") }),
    /sourceAdapterEngineDeploymentHash must not be zero/u,
  );
  assert.throws(
    () =>
      buildEthereumMainnetSccpLocalAdmissionSubmission({
        ...input,
        envelopeEncoding: "abi_tuple_v1",
      }),
    /metadata is not canonical/u,
  );
  assert.throws(
    () =>
      buildEthereumMainnetSccpLocalAdmissionSubmission({
        ...input,
        proofFamily: "debug-proof-family",
      }),
    /metadata is not canonical/u,
  );
});

test("EthereumMainnetSccp inbound proving rejects foreign EVM domains before callbacks run", async () => {
  let called = false;
  const sdk = new EthereumMainnetSccp({
    proveInbound() {
      called = true;
    },
  });

  await assert.rejects(
    () =>
      sdk.proveInboundToSora({
        sourceDomain: SCCP_DOMAIN_BSC,
        targetDomain: SCCP_DOMAIN_SORA,
      }),
    /sourceDomain must be ETH/u,
  );
  assert.equal(called, false);
});
