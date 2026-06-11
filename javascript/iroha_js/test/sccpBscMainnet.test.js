import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  BscMainnetSccp,
  BscTestnetSccp,
  BscTestnetSccpProver,
  SCCP_BSC_MAINNET_EVM_CHAIN_ID,
  SCCP_BSC_MAINNET_NETWORK_ID,
  SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
  SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
  SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
  SCCP_BSC_TESTNET_EVM_CHAIN_ID,
  SCCP_BSC_TESTNET_NETWORK_ID,
  SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
  SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
  SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_ETH,
  SCCP_DOMAIN_SORA,
  SCCP_CODEC_EVM_HEX,
  SCCP_CODEC_TEXT_UTF8,
  SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
  SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
  SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1,
  SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
  SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1,
  SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1,
  SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
  SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1,
  SCCP_TAIRA_XOR_ASSET_KEY_V1,
  bscMainnetSccpDestinationBinding,
  bscTestnetSccpDestinationBinding,
  bscSccpReceiptProofHash,
  buildBscMainnetSccpLocalAdmissionSubmission,
  buildBscMainnetSccpDestinationProofRequest,
  buildBscTestnetSccpLocalAdmissionSubmission,
  buildBscTestnetSccpDestinationProofRequest,
  buildBscTestnetSccpDestinationSubmission,
  canonicalSccpMessageProofBundleBytes,
  canonicalSccpPayloadEnvelopeBytes,
  evmSccpSourceEventTopic,
  parseBscMainnetNativeEvmProverBundleManifest,
  parseBscMainnetNativeEvmProverParityFixture,
  parseBscMainnetNativeEvmProverSelfTestFixture,
  parseBscTestnetNativeEvmProverBundleManifest,
  parseBscTestnetNativeEvmProverParityFixture,
  parseBscTestnetNativeEvmProverSelfTestFixture,
  runBscTestnetNativeProverSelfTest,
  validateBscMainnetNativeEvmProverBundle,
  validateBscMainnetNativeEvmProverParityFixture,
  validateBscMainnetNativeEvmProverSelfTestFixture,
  validateBscTestnetNativeEvmProverBundle,
  validateBscTestnetNativeEvmProverParityFixture,
  validateBscTestnetNativeEvmProverSelfTestFixture,
  verifyBscMainnetNativeEvmProverArtifacts,
  verifyBscTestnetNativeEvmProverArtifacts,
  verifyBscTestnetNativeEvmProverArtifactsFromBundle,
  sccpMerkleRootFromCommitment,
  sccpPayloadHash,
  sccpTransferMessageId,
  wrapBscMainnetSccpDestinationProofResult,
  wrapBscTestnetSccpDestinationProofResult,
} from "../src/sccp.js";

const hex32 = (byte) => `0x${byte.repeat(32)}`;
const sha256Hex = (bytes) =>
  `0x${createHash("sha256").update(Buffer.from(bytes)).digest("hex")}`;
const TX_HASH = hex32("aa");
const BLOCK_HASH = hex32("bb");
const RECEIPTS_ROOT = hex32("cc");
const SOURCE_EVENT_DIGEST = hex32("34");
const SOURCE_BRIDGE_ADDRESS = `0x${"44".repeat(20)}`;

const sampleParliaFinality = (overrides = {}) => ({
  executionBlockNumber: "0x1234",
  executionBlockHash: BLOCK_HASH,
  executionReceiptsRoot: RECEIPTS_ROOT,
  validatorEpoch: "0x24",
  commitSealHash: hex32("dd"),
  ...overrides,
});

const sampleInboundEvidence = () => ({
  sourceDomain: SCCP_DOMAIN_BSC,
  targetDomain: SCCP_DOMAIN_SORA,
  sourceBridgeEmitterAddress: SOURCE_BRIDGE_ADDRESS,
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
    receiptsRoot: RECEIPTS_ROOT,
  },
  parliaFinality: sampleParliaFinality(),
});

const sampleReceiptProof = {
  sourceDomain: SCCP_DOMAIN_BSC,
  sourceEventDigest: SOURCE_EVENT_DIGEST,
  validatorEpoch: "36",
  blockNumber: "4660",
  blockHash: BLOCK_HASH,
  receiptsRoot: RECEIPTS_ROOT,
  validatorSetHash: hex32("ef"),
  commitSealHash: hex32("dd"),
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

const buildSampleOutboundBundleFixture = ({
  targetDomain = SCCP_DOMAIN_BSC,
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

const destinationBindingInputFromBinding = (binding) => ({
  verifierAddress: binding.verifierAddress,
  bridgeAddress: binding.bridgeAddress,
  verifierCodeHash: binding.verifierCodeHash,
  verifierKeyHash: binding.verifierKeyHash,
});

const sampleOutboundInput = (
  targetDomain = SCCP_DOMAIN_BSC,
  destinationBindingOverrides = {},
) => {
  const fixture =
    targetDomain === SCCP_DOMAIN_BSC
      ? sampleOutboundFixture
      : buildSampleOutboundBundleFixture({ targetDomain });
  return {
    publicInputs: { ...fixture.publicInputs },
    bundleBytes: fixture.bundleBytes,
    destinationBinding: bscMainnetSccpDestinationBinding(
      sampleDestinationBindingInput(destinationBindingOverrides),
    ),
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: hex32("66"),
  };
};

const sampleBscTestnetOutboundInput = (
  targetDomain = SCCP_DOMAIN_BSC,
  destinationBindingOverrides = {},
) => {
  const fixture =
    targetDomain === SCCP_DOMAIN_BSC
      ? sampleOutboundFixture
      : buildSampleOutboundBundleFixture({ targetDomain });
  return {
    publicInputs: { ...fixture.publicInputs },
    bundleBytes: fixture.bundleBytes,
    destinationBinding: bscTestnetSccpDestinationBinding(
      sampleDestinationBindingInput(destinationBindingOverrides),
    ),
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: hex32("66"),
  };
};

const sampleBscTestnetNativeEvmProverBundle = (
  destinationBindingHash,
  overrides = {},
) => {
  const proofArtifactHash = hex32("91");
  const provingKeyHash = hex32("92");
  return {
    schema: SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
    bundle_id: SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
    domain: SCCP_DOMAIN_BSC,
    chain: "bsc-testnet",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact: "artifacts/bsc-testnet/proof-artifact.r1cs",
    proof_artifact_hash: proofArtifactHash,
    proving_key: "artifacts/bsc-testnet/proving-key.zkey",
    proving_key_hash: provingKeyHash,
    verifier_key: "artifacts/bsc-testnet/verifier-key.bin",
    verifier_key_hash: hex32("cc"),
    destination_binding_hash: destinationBindingHash,
    no_wasm: true,
    remote_prover_required: false,
    browser_implementation: "pure-typescript",
    cross_sdk_fixture_parity_artifact:
      "artifacts/bsc-testnet/cross-sdk-fixture-parity.json",
    native_prover_self_test_artifact:
      "artifacts/bsc-testnet/native-prover-self-test.json",
    native_sdk_artifacts: Object.entries(
      SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
    ).map(([sdk, implementation], index) => ({
      sdk,
      implementation,
      prover_artifact_hash: proofArtifactHash,
      proving_key_hash: provingKeyHash,
      implementation_artifact: `artifacts/bsc-testnet/${sdk}-implementation.bin`,
      implementation_hash: hex32((index + 1).toString(16).padStart(2, "0")),
    })),
    audit_hashes: {
      circuit_security_audit: hex32("a1"),
      native_implementation_audit: hex32("a2"),
      reproducible_build_attestation: hex32("a3"),
      cross_sdk_fixture_parity: hex32("a4"),
      native_prover_self_test: hex32("a5"),
      no_wasm_no_remote_scan: hex32("a6"),
    },
    ...overrides,
  };
};

const sampleBscTestnetNativeEvmProverParityFixture = (
  bundle,
  overrides = {},
) => {
  const publicSignalWords = Array.from({ length: 9 }, (_, index) =>
    hex32((index + 0x10).toString(16).padStart(2, "0")),
  );
  const destinationBindingHash =
    bundle.destination_binding_hash ?? bundle.destinationBindingHash;
  const result = {
    receipt_proof_hash: hex32("d1"),
    source_proof_hash: hex32("d2"),
    destination_binding_hash: destinationBindingHash,
    public_signal_words: publicSignalWords,
    calldata_hash: hex32("d3"),
    torii_submit_payload_hash: hex32("d4"),
  };
  return {
    schema: SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
    domain: SCCP_DOMAIN_BSC,
    chain: "bsc-testnet",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact_hash: bundle.proof_artifact_hash ?? bundle.proofArtifactHash,
    proving_key_hash: bundle.proving_key_hash ?? bundle.provingKeyHash,
    verifier_key_hash: bundle.verifier_key_hash ?? bundle.verifierKeyHash,
    destination_binding_hash: destinationBindingHash,
    ...result,
    sdk_results: Object.fromEntries(
      Object.keys(SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1).map(
        (sdk) => [sdk, { ...result }],
      ),
    ),
    ...overrides,
  };
};

const sampleBscTestnetNativeEvmProverParityFixtureBytes = (
  bundle,
  overrides = {},
) =>
  Buffer.from(
    JSON.stringify(
      sampleBscTestnetNativeEvmProverParityFixture(bundle, overrides),
    ),
    "utf8",
  );

const sampleBscTestnetNativeEvmProverSelfTestFixture = (
  bundle,
  overrides = {},
) => {
  const publicSignalWords = Array.from({ length: 9 }, (_, index) =>
    hex32((index + 0x30).toString(16).padStart(2, "0")),
  );
  const destinationBindingHash =
    bundle.destination_binding_hash ?? bundle.destinationBindingHash;
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
    schema: SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
    domain: SCCP_DOMAIN_BSC,
    chain: "bsc-testnet",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact_hash: bundle.proof_artifact_hash ?? bundle.proofArtifactHash,
    proving_key_hash: bundle.proving_key_hash ?? bundle.provingKeyHash,
    verifier_key_hash: bundle.verifier_key_hash ?? bundle.verifierKeyHash,
    destination_binding_hash: destinationBindingHash,
    ...result,
    sdk_results: Object.fromEntries(
      Object.keys(SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1).map(
        (sdk) => [sdk, { ...result }],
      ),
    ),
    ...overrides,
  };
};

const sampleBscTestnetNativeEvmProverSelfTestFixtureBytes = (
  bundle,
  overrides = {},
) =>
  Buffer.from(
    JSON.stringify(
      sampleBscTestnetNativeEvmProverSelfTestFixture(bundle, overrides),
    ),
    "utf8",
  );

const sampleBscTestnetNativeEvmProverBundleWithFixtureBytes = (
  destinationBindingHash,
  overrides = {},
) => {
  const draftBundle = sampleBscTestnetNativeEvmProverBundle(
    destinationBindingHash,
    overrides,
  );
  const parityFixtureBytes =
    sampleBscTestnetNativeEvmProverParityFixtureBytes(draftBundle);
  const parityFixtureHash = sha256Hex(parityFixtureBytes);
  const selfTestFixtureBytes =
    sampleBscTestnetNativeEvmProverSelfTestFixtureBytes(draftBundle);
  const selfTestFixtureHash = sha256Hex(selfTestFixtureBytes);
  const bundle = sampleBscTestnetNativeEvmProverBundle(
    destinationBindingHash,
    {
      ...overrides,
      audit_hashes: {
        ...draftBundle.audit_hashes,
        cross_sdk_fixture_parity: parityFixtureHash,
        native_prover_self_test: selfTestFixtureHash,
      },
    },
  );
  return {
    bundle,
    parityFixtureBytes:
      sampleBscTestnetNativeEvmProverParityFixtureBytes(bundle),
    parityFixtureHash,
    selfTestFixtureBytes:
      sampleBscTestnetNativeEvmProverSelfTestFixtureBytes(bundle),
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

const sampleVerifiedBscTestnetNativeEvmProverFixture = () => {
  const proofArtifactBytes = nativeEvmSnarkjsArtifactBytes(
    "sccp bsc testnet proof artifact v1",
    "r1cs",
    3,
  );
  const provingKeyBytes = nativeEvmSnarkjsArtifactBytes(
    "sccp bsc testnet proving key v1",
    "zkey",
    10,
  );
  const verifierKeyBytes = nativeEvmProverArtifactBytes(
    "sccp bsc testnet verifier key v1",
  );
  const implementationBytes = nativeEvmProverArtifactBytes(
    "sccp bsc testnet pure typescript prover artifact v1",
  );
  const proofArtifactHash = sha256Hex(proofArtifactBytes);
  const provingKeyHash = sha256Hex(provingKeyBytes);
  const verifierKeyHash = sha256Hex(verifierKeyBytes);
  const implementationHash = sha256Hex(implementationBytes);
  const destinationBinding = bscTestnetSccpDestinationBinding(
    sampleDestinationBindingInput({ verifierKeyHash }),
  );
  const {
    bundle,
    parityFixtureBytes,
    parityFixtureHash,
    selfTestFixtureBytes,
    selfTestFixtureHash,
  } = sampleBscTestnetNativeEvmProverBundleWithFixtureBytes(
    destinationBinding.bindingHash,
    {
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
        implementation_artifact: `artifacts/bsc-testnet/${sdk}-implementation.bin`,
        implementation_hash:
          sdk === "javascript"
            ? implementationHash
            : hex32((index + 1).toString(16).padStart(2, "0")),
      })),
    },
  );
  const nativeProverArtifacts = verifyBscTestnetNativeEvmProverArtifacts(
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
  );
  return {
    bundle,
    destinationBinding,
    nativeProverArtifacts,
    proofArtifactBytes,
    provingKeyBytes,
    verifierKeyBytes,
    parityFixtureBytes,
    parityFixtureHash,
    selfTestFixtureBytes,
    selfTestFixtureHash,
    implementationBytes,
    implementationHash,
  };
};

const sampleVerifiedBscMainnetNativeEvmProverFixture = () => {
  const proofArtifactBytes = nativeEvmSnarkjsArtifactBytes(
    "sccp bsc mainnet proof artifact v1",
    "r1cs",
    3,
  );
  const provingKeyBytes = nativeEvmSnarkjsArtifactBytes(
    "sccp bsc mainnet proving key v1",
    "zkey",
    10,
  );
  const verifierKeyBytes = nativeEvmProverArtifactBytes(
    "sccp bsc mainnet verifier key v1",
  );
  const implementationBytes = nativeEvmProverArtifactBytes(
    "sccp bsc mainnet pure typescript prover artifact v1",
  );
  const proofArtifactHash = sha256Hex(proofArtifactBytes);
  const provingKeyHash = sha256Hex(provingKeyBytes);
  const verifierKeyHash = sha256Hex(verifierKeyBytes);
  const implementationHash = sha256Hex(implementationBytes);
  const destinationBinding = bscMainnetSccpDestinationBinding(
    sampleDestinationBindingInput({ verifierKeyHash }),
  );
  const bundleBase = {
    bundle_id: SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
    chain: "bsc-mainnet",
    proof_artifact: "artifacts/bsc-mainnet/proof-artifact.r1cs",
    proof_artifact_hash: proofArtifactHash,
    proving_key: "artifacts/bsc-mainnet/proving-key.zkey",
    proving_key_hash: provingKeyHash,
    verifier_key: "artifacts/bsc-mainnet/verifier-key.bin",
    verifier_key_hash: verifierKeyHash,
    cross_sdk_fixture_parity_artifact:
      "artifacts/bsc-mainnet/cross-sdk-fixture-parity.json",
    native_prover_self_test_artifact:
      "artifacts/bsc-mainnet/native-prover-self-test.json",
    native_sdk_artifacts: Object.entries(
      SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
    ).map(([sdk, implementation], index) => ({
      sdk,
      implementation,
      prover_artifact_hash: proofArtifactHash,
      proving_key_hash: provingKeyHash,
      implementation_artifact: `artifacts/bsc-mainnet/${sdk}-implementation.bin`,
      implementation_hash:
        sdk === "javascript"
          ? implementationHash
          : hex32((index + 0x41).toString(16).padStart(2, "0")),
    })),
  };
  const draftBundle = sampleBscTestnetNativeEvmProverBundle(
    destinationBinding.bindingHash,
    bundleBase,
  );
  const parityFixtureBytes = Buffer.from(
    JSON.stringify(
      sampleBscTestnetNativeEvmProverParityFixture(draftBundle, {
        schema: SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
        chain: "bsc-mainnet",
      }),
    ),
    "utf8",
  );
  const selfTestFixtureBytes = Buffer.from(
    JSON.stringify(
      sampleBscTestnetNativeEvmProverSelfTestFixture(draftBundle, {
        schema: SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
        chain: "bsc-mainnet",
      }),
    ),
    "utf8",
  );
  const parityFixtureHash = sha256Hex(parityFixtureBytes);
  const selfTestFixtureHash = sha256Hex(selfTestFixtureBytes);
  const bundle = sampleBscTestnetNativeEvmProverBundle(
    destinationBinding.bindingHash,
    {
      ...bundleBase,
      audit_hashes: {
        ...draftBundle.audit_hashes,
        cross_sdk_fixture_parity: parityFixtureHash,
        native_prover_self_test: selfTestFixtureHash,
      },
    },
  );
  const nativeProverArtifacts = verifyBscMainnetNativeEvmProverArtifacts(
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
  );
  return {
    bundle,
    destinationBinding,
    nativeProverArtifacts,
    proofArtifactBytes,
    provingKeyBytes,
    verifierKeyBytes,
    parityFixtureBytes,
    parityFixtureHash,
    selfTestFixtureBytes,
    selfTestFixtureHash,
    implementationBytes,
    implementationHash,
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

test("BscMainnetSccp validates EIP-1193 execution providers as BSC mainnet", async () => {
  const provider = {
    async request({ method }) {
      assert.equal(method, "eth_chainId");
      return "0x38";
    },
  };
  const sdk = new BscMainnetSccp({ executionProvider: provider });

  assert.equal(await sdk.validateExecutionProviderMainnet(), "0x38");
  assert.equal(SCCP_BSC_MAINNET_EVM_CHAIN_ID, 56);
});

test("BscMainnetSccp rejects Ethereum and noncanonical JSON-RPC chain ids", async () => {
  await assert.rejects(
    () =>
      new BscMainnetSccp({
        executionProvider: {
          async request() {
            return "0x1";
          },
        },
      }).validateExecutionProviderMainnet(),
    /eth_chainId == 0x38/u,
  );
  await assert.rejects(
    () =>
      new BscMainnetSccp({
        executionProvider: {
          async request() {
            return "0x038";
          },
        },
      }).validateExecutionProviderMainnet(),
    /canonical JSON-RPC quantity/u,
  );
  await assert.rejects(
    () =>
      new BscMainnetSccp({
        executionProvider: {
          async request() {
            return "56";
          },
        },
      }).validateExecutionProviderMainnet(),
    /canonical JSON-RPC quantity/u,
  );
  await assert.rejects(
    () =>
      new BscMainnetSccp({
        executionProvider: {
          async request() {
            return 56;
          },
        },
      }).validateExecutionProviderMainnet(),
    /canonical JSON-RPC quantity/u,
  );
});

test("BscTestnetSccp validates EIP-1193 execution providers as BSC testnet", async () => {
  const provider = {
    async request({ method }) {
      assert.equal(method, "eth_chainId");
      return "0x61";
    },
  };
  const sdk = new BscTestnetSccp({ executionProvider: provider });

  assert.equal(await sdk.validateExecutionProviderTestnet(), "0x61");
  assert.equal(SCCP_BSC_TESTNET_EVM_CHAIN_ID, 97);

  await assert.rejects(
    () =>
      new BscTestnetSccp({
        executionProvider: {
          async request() {
            return "0x38";
          },
        },
      }).validateExecutionProviderTestnet(),
    /eth_chainId == 0x61/u,
  );
  await assert.rejects(
    () =>
      new BscTestnetSccp({
        executionProvider: {
          async request() {
            return "0x061";
          },
        },
      }).validateExecutionProviderTestnet(),
    /canonical JSON-RPC quantity/u,
  );
});

test("BscMainnetSccp collects receipt evidence from BSC execution and Parlia providers", async () => {
  const calls = [];
  const provider = {
    async request({ method, params }) {
      calls.push([method, params]);
      if (method === "eth_chainId") return "0x38";
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
        return { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: RECEIPTS_ROOT };
      }
      throw new Error(`unexpected RPC method ${method}`);
    },
  };
  const consensusProvider = {
    async collectFinalityEvidence({ receipt, block, transactionHash }) {
      assert.equal(transactionHash, TX_HASH);
      assert.equal(receipt.blockHash, BLOCK_HASH);
      assert.equal(block.hash, BLOCK_HASH);
      return sampleParliaFinality();
    },
  };
  const sdk = new BscMainnetSccp({ executionProvider: provider, consensusProvider });

  const evidence = await sdk.collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH });

  assert.equal(evidence.sourceDomain, SCCP_DOMAIN_BSC);
  assert.equal(evidence.targetDomain, SCCP_DOMAIN_SORA);
  assert.equal(evidence.receipt.blockHash, BLOCK_HASH);
  assert.equal(evidence.block.hash, BLOCK_HASH);
  assert.equal(evidence.parliaFinality.executionBlockNumber, "4660");
  assert.equal(evidence.parliaFinality.executionBlockHash, BLOCK_HASH);
  assert.equal(evidence.parliaFinality.executionReceiptsRoot, RECEIPTS_ROOT);
  assert.equal(evidence.parliaFinality.commitSealHash, hex32("dd"));
  assert.deepEqual(
    calls.map(([method]) => method),
    ["eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"],
  );
});

test("BscMainnetSccp collectInboundEvidenceFromReceipt snapshots consensus evidence", async () => {
  const mutableTopics = [evmSccpSourceEventTopic(), SOURCE_EVENT_DIGEST];
  const receiptLogs = [sourceEventLog({ topics: mutableTopics })];
  const receipt = {
    transactionHash: TX_HASH,
    blockHash: BLOCK_HASH,
    blockNumber: "0x1234",
    status: "0x1",
    logs: receiptLogs,
  };
  const blockWitness = { branch: [hex32("e1")], bytes: new Uint8Array([0xbb]) };
  const block = {
    hash: BLOCK_HASH,
    number: "0x1234",
    receiptsRoot: RECEIPTS_ROOT,
    mutableWitness: blockWitness,
  };
  const finalityWitness = {
    branch: [hex32("e2")],
    bytes: new Uint8Array([0xcc]),
  };
  const mutablePayload = new Uint8Array([0xaa]);
  const sdk = new BscMainnetSccp({
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
        return sampleParliaFinality({ mutableWitness: finalityWitness });
      },
    },
  });

  const evidence = await sdk.collectInboundEvidenceFromReceipt({
    receipt,
    block,
    mutablePayload,
  });
  finalityWitness.branch.push(hex32("99"));
  finalityWitness.bytes[0] = 0x7d;
  mutablePayload[0] = 0x7e;

  assert.equal(Object.isFrozen(evidence), true);
  assert.equal(Object.isFrozen(evidence.receipt.logs), true);
  assert.equal(Object.isFrozen(evidence.parliaFinality.mutableWitness.branch), true);
  assert.throws(() => {
    evidence.receipt.logs.push(sourceEventLog());
  }, TypeError);
  assert.equal(evidence.mutablePayload[0], 0xaa);
  assert.equal(evidence.receipt.logs.length, 1);
  assert.equal(evidence.receipt.logs[0].topics[1], SOURCE_EVENT_DIGEST);
  assert.deepEqual(evidence.block.mutableWitness.branch, [hex32("e1")]);
  assert.deepEqual([...evidence.block.mutableWitness.bytes], [0xbb]);
  assert.deepEqual(evidence.parliaFinality.mutableWitness.branch, [hex32("e2")]);
  assert.deepEqual([...evidence.parliaFinality.mutableWitness.bytes], [0xcc]);
});

test("BscMainnetSccp rejects failed or drifted receipt evidence before proving", async () => {
  const providerForReceipt = (
    receipt,
    block = { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: RECEIPTS_ROOT },
  ) => ({
    async request({ method }) {
      if (method === "eth_chainId") return "0x38";
      if (method === "eth_getTransactionReceipt") return receipt;
      if (method === "eth_getBlockByHash") return block;
      throw new Error(`unexpected RPC method ${method}`);
    },
  });

  await assert.rejects(
    () =>
      new BscMainnetSccp({
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
      new BscMainnetSccp({
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
      new BscMainnetSccp({
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
      new BscMainnetSccp({
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
      new BscMainnetSccp({
        executionProvider: providerForReceipt(
          {
            transactionHash: TX_HASH,
            blockHash: BLOCK_HASH,
            blockNumber: "0x1234",
            status: "0x1",
          },
          { hash: hex32("bc"), number: "0x1234", receiptsRoot: RECEIPTS_ROOT },
        ),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /block.hash must match receipt.blockHash/u,
  );

  await assert.rejects(
    () =>
      new BscMainnetSccp({
        executionProvider: providerForReceipt(
          {
            transactionHash: TX_HASH,
            blockHash: BLOCK_HASH,
            blockNumber: "0x1234",
            status: "0x1",
          },
          { hash: BLOCK_HASH, receiptsRoot: RECEIPTS_ROOT },
        ),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /block\.number is required/u,
  );

  await assert.rejects(
    () =>
      new BscMainnetSccp({
        executionProvider: providerForReceipt(
          {
            transactionHash: TX_HASH,
            blockHash: BLOCK_HASH,
            blockNumber: "0x1234",
            status: "0x1",
          },
          { hash: BLOCK_HASH, number: "0x0", receiptsRoot: RECEIPTS_ROOT },
        ),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /block\.number must be positive/u,
  );

  await assert.rejects(
    () =>
      new BscMainnetSccp({
        executionProvider: providerForReceipt(
          {
            transactionHash: TX_HASH,
            blockHash: BLOCK_HASH,
            blockNumber: "0x1234",
            status: "0x1",
          },
          { hash: BLOCK_HASH, number: "0x1235", receiptsRoot: RECEIPTS_ROOT },
        ),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /block.number must match receipt.blockNumber/u,
  );

  await assert.rejects(
    () =>
      new BscMainnetSccp({
        executionProvider: providerForReceipt({
          transactionHash: TX_HASH.toUpperCase(),
          blockHash: BLOCK_HASH,
          blockNumber: "0x1234",
          status: "0x1",
        }),
      }).collectInboundEvidenceFromReceipt({ transactionHash: TX_HASH }),
    /canonical lowercase/u,
  );
});

test("BscMainnetSccp keeps the easy outbound path BSC-only", () => {
  const sdk = new BscMainnetSccp();
  const request = sdk.buildOutboundProofRequest(sampleOutboundInput());
  assert.equal(request.targetDomain, SCCP_DOMAIN_BSC);
  assert.equal(request.destinationBinding.networkId, SCCP_BSC_MAINNET_NETWORK_ID);

  assert.throws(
    () => sdk.buildOutboundProofRequest(sampleOutboundInput(SCCP_DOMAIN_ETH)),
    /request route|targetDomain|BSC/u,
  );
});

test("BSC testnet destination helpers bind outbound proofs to chain id 97", () => {
  const testnetBinding = bscTestnetSccpDestinationBinding(sampleDestinationBindingInput());
  assert.equal(SCCP_BSC_TESTNET_EVM_CHAIN_ID, 97);
  assert.equal(testnetBinding.sourceDomain, SCCP_DOMAIN_SORA);
  assert.equal(testnetBinding.targetDomain, SCCP_DOMAIN_BSC);
  assert.equal(testnetBinding.networkId, SCCP_BSC_TESTNET_NETWORK_ID);

  const input = {
    publicInputs: samplePublicInputs,
    bundleBytes: sampleOutboundFixture.bundleBytes,
    destinationBinding: testnetBinding,
    sourceDomain: SCCP_DOMAIN_SORA,
    statementHash: hex32("66"),
  };
  const request = buildBscTestnetSccpDestinationProofRequest(input);
  assert.equal(request.targetDomain, SCCP_DOMAIN_BSC);
  assert.equal(request.destinationBinding.networkId, SCCP_BSC_TESTNET_NETWORK_ID);

  const proofResult = wrapBscTestnetSccpDestinationProofResult(
    groth16ProofBytes(request.publicInputs),
    request,
  );
  const submission = buildBscTestnetSccpDestinationSubmission({ proofResult });
  assert.equal(submission.targetDomain, SCCP_DOMAIN_BSC);
  assert.equal(submission.destinationBindingHash, request.destinationBindingHash);
  const tamperedBscBase64ProofResult = {
    ...proofResult,
    proofBase64: "AAAA",
  };
  const sdk = new BscTestnetSccp();
  assert.throws(
    () => sdk.buildBscCalldata({ proofResult: tamperedBscBase64ProofResult }),
    /proofResult\.proofBase64 must match proofResult\.proofBytes/u,
  );

  assert.throws(
    () =>
      bscTestnetSccpDestinationBinding(
        sampleDestinationBindingInput({ networkId: SCCP_BSC_MAINNET_NETWORK_ID }),
      ),
    /chain id 97/u,
  );
  assert.throws(
    () =>
      buildBscTestnetSccpDestinationProofRequest({
        ...input,
        ...sampleBscTestnetOutboundInput(SCCP_DOMAIN_ETH),
        destinationBinding: testnetBinding,
      }),
    /destinationBinding must match request route|target BSC/u,
  );
  const mainnetRequest = buildBscMainnetSccpDestinationProofRequest(sampleOutboundInput());
  const mainnetProofResult = wrapBscMainnetSccpDestinationProofResult(
    groth16ProofBytes(mainnetRequest.publicInputs),
    mainnetRequest,
  );
  assert.throws(
    () => buildBscTestnetSccpDestinationSubmission({ proofResult: mainnetProofResult }),
    /BSC testnet destinationBinding|chain id 97/u,
  );
});

test("BscTestnetSccp keeps outbound proof and calldata on BSC testnet", async () => {
  const { destinationBinding, nativeProverArtifacts } =
    sampleVerifiedBscTestnetNativeEvmProverFixture();
  const sdk = new BscTestnetSccp({ nativeProverArtifacts });
  const request = sdk.buildOutboundProofRequest(
    sampleBscTestnetOutboundInput(
      SCCP_DOMAIN_BSC,
      destinationBindingInputFromBinding(destinationBinding),
    ),
  );
  assert.equal(request.targetDomain, SCCP_DOMAIN_BSC);
  assert.equal(request.destinationBinding.networkId, SCCP_BSC_TESTNET_NETWORK_ID);

  assert.throws(
    () => sdk.buildOutboundProofRequest(sampleBscTestnetOutboundInput(SCCP_DOMAIN_ETH)),
    /request route|targetDomain|BSC/u,
  );

  const proofResult = wrapBscTestnetSccpDestinationProofResult(
    groth16ProofBytes(request.publicInputs),
    request,
  );
  const submission = sdk.buildBscCalldata({ proofResult });
  assert.equal(submission.targetDomain, SCCP_DOMAIN_BSC);
  assert.equal(submission.destinationBindingHash, request.destinationBindingHash);

  const mainnetRequest = buildBscMainnetSccpDestinationProofRequest(sampleOutboundInput());
  const mainnetProofResult = wrapBscMainnetSccpDestinationProofResult(
    groth16ProofBytes(mainnetRequest.publicInputs),
    mainnetRequest,
  );
  assert.throws(
    () => sdk.buildBscCalldata({ proofResult: mainnetProofResult }),
    /chain id 97|BSC testnet destinationBinding/u,
  );

  assert.equal((await new BscTestnetSccpProver().buildRequest(
    sampleBscTestnetOutboundInput(),
  )).destinationBinding.networkId, SCCP_BSC_TESTNET_NETWORK_ID);
});

test("BscTestnetSccp validates native prover bundles and binds artifact hashes", async () => {
  const fixture = sampleVerifiedBscTestnetNativeEvmProverFixture();
  const descriptor = validateBscTestnetNativeEvmProverBundle(fixture.bundle, {
    destinationBinding: fixture.destinationBinding,
  });
  const parsedDescriptor = parseBscTestnetNativeEvmProverBundleManifest(
    JSON.stringify(fixture.bundle),
    { destinationBinding: fixture.destinationBinding },
  );

  assert.deepEqual(parsedDescriptor, descriptor);
  assert.equal(descriptor.bundleId, SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1);
  assert.equal(descriptor.domain, SCCP_DOMAIN_BSC);
  assert.equal(descriptor.chain, "bsc-testnet");
  assert.equal(descriptor.destinationBindingHash, fixture.destinationBinding.bindingHash);

  const parityFixture =
    sampleBscTestnetNativeEvmProverParityFixture(fixture.bundle);
  const parityDescriptor = validateBscTestnetNativeEvmProverParityFixture(
    parityFixture,
    fixture.bundle,
  );
  assert.deepEqual(
    parseBscTestnetNativeEvmProverParityFixture(
      JSON.stringify(parityFixture),
      fixture.bundle,
    ),
    parityDescriptor,
  );
  assert.equal(
    parityDescriptor.schema,
    SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
  );

  const selfTestFixture =
    sampleBscTestnetNativeEvmProverSelfTestFixture(fixture.bundle);
  const selfTestDescriptor = validateBscTestnetNativeEvmProverSelfTestFixture(
    selfTestFixture,
    fixture.bundle,
  );
  assert.deepEqual(
    parseBscTestnetNativeEvmProverSelfTestFixture(
      JSON.stringify(selfTestFixture),
      fixture.bundle,
    ),
    selfTestDescriptor,
  );
  assert.equal(
    selfTestDescriptor.schema,
    SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
  );

  const javascriptArtifact = descriptor.nativeSdkArtifacts.find(
    (artifact) => artifact.sdk === "javascript",
  );
  const artifactBytesByPath = new Map([
    [descriptor.proofArtifact, fixture.proofArtifactBytes],
    [descriptor.provingKey, fixture.provingKeyBytes],
    [descriptor.verifierKey, fixture.verifierKeyBytes],
    [descriptor.crossSdkFixtureParityArtifact, fixture.parityFixtureBytes],
    [descriptor.nativeProverSelfTestArtifact, fixture.selfTestFixtureBytes],
    [javascriptArtifact.implementationArtifact, fixture.implementationBytes],
  ]);
  const verifiedFromBundle =
    await verifyBscTestnetNativeEvmProverArtifactsFromBundle(
      {
        nativeProverBundle: JSON.stringify(fixture.bundle),
        sdk: "javascript",
        artifactResolver(path) {
          return artifactBytesByPath.get(path);
        },
      },
      { destinationBinding: fixture.destinationBinding },
    );
  assert.equal(verifiedFromBundle.implementationHash, fixture.implementationHash);
  assert.equal(verifiedFromBundle.nativeProverBundle.chain, "bsc-testnet");

  const selfTestResult = await runBscTestnetNativeProverSelfTest({
    nativeProverArtifacts: verifiedFromBundle,
    nativeProverSelfTest(context) {
      assert.equal(context.sdk, "javascript");
      return context.expectedResult;
    },
  });
  assert.equal(selfTestResult.proofHash, hex32("e4"));

  let factoryRequest;
  const sdk = await BscTestnetSccp.fromNativeProverBundle({
    destinationBinding: fixture.destinationBinding,
    manifest: JSON.stringify(fixture.bundle),
    sdk: "javascript",
    artifactResolver(path) {
      return artifactBytesByPath.get(path);
    },
    nativeProverSelfTest(context) {
      return context.expectedResult;
    },
    outboundProver: {
      async prove(request) {
        factoryRequest = request;
        return wrapBscTestnetSccpDestinationProofResult(
          groth16ProofBytes(request.publicInputs),
          request,
        );
      },
    },
  });
  const proofResult = await sdk.proveOutboundToBsc(
    sampleBscTestnetOutboundInput(
      SCCP_DOMAIN_BSC,
      destinationBindingInputFromBinding(fixture.destinationBinding),
    ),
  );
  assert.equal(factoryRequest.proofArtifactHash, descriptor.proofArtifactHash);
  assert.equal(proofResult.provingKeyHash, descriptor.provingKeyHash);
  assert.equal(sdk.buildBscCalldata({ proofResult }).targetDomain, SCCP_DOMAIN_BSC);

  assert.throws(
    () =>
      validateBscTestnetNativeEvmProverBundle(
        { ...fixture.bundle, chain: "eth" },
        { destinationBinding: fixture.destinationBinding },
      ),
    /chain must be bsc-testnet/u,
  );
});

test("BscTestnetSccp rejects native prover bundle artifact path aliasing", () => {
  const fixture = sampleVerifiedBscTestnetNativeEvmProverFixture();

  assert.throws(
    () =>
      validateBscTestnetNativeEvmProverBundle(
        {
          ...fixture.bundle,
          verifier_key: fixture.bundle.proof_artifact,
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /artifact paths must be role-separated: verifierKey reuses proofArtifact/u,
  );

  const sdkArtifacts = fixture.bundle.native_sdk_artifacts.map((artifact) => ({
    ...artifact,
  }));
  sdkArtifacts[1].implementation_artifact =
    sdkArtifacts[0].implementation_artifact;

  assert.throws(
    () =>
      validateBscTestnetNativeEvmProverBundle(
        {
          ...fixture.bundle,
          native_sdk_artifacts: sdkArtifacts,
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /nativeSdkArtifacts\[.*\]\.implementationArtifact reuses nativeSdkArtifacts\[.*\]\.implementationArtifact/u,
  );
});

test("BscTestnetSccp rejects native prover bundle proof material with generic file extensions", () => {
  const fixture = sampleVerifiedBscTestnetNativeEvmProverFixture();

  assert.throws(
    () =>
      validateBscTestnetNativeEvmProverBundle(
        {
          ...fixture.bundle,
          proof_artifact: "artifacts/bsc-testnet/proof-artifact.bin",
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /proofArtifact must reference a \.r1cs artifact/u,
  );

  assert.throws(
    () =>
      validateBscTestnetNativeEvmProverBundle(
        {
          ...fixture.bundle,
          proving_key: "artifacts/bsc-testnet/proving-key.bin",
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /provingKey must reference a \.zkey artifact/u,
  );
});

test("BscTestnetSccp rejects native prover bundle JSON support artifacts with generic file extensions", () => {
  const fixture = sampleVerifiedBscTestnetNativeEvmProverFixture();

  assert.throws(
    () =>
      validateBscTestnetNativeEvmProverBundle(
        {
          ...fixture.bundle,
          cross_sdk_fixture_parity_artifact:
            "artifacts/bsc-testnet/cross-sdk-fixture-parity.fixture",
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /crossSdkFixtureParityArtifact must reference a \.json artifact/u,
  );

  assert.throws(
    () =>
      validateBscTestnetNativeEvmProverBundle(
        {
          ...fixture.bundle,
          native_prover_self_test_artifact:
            "artifacts/bsc-testnet/native-prover-self-test.fixture",
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /nativeProverSelfTestArtifact must reference a \.json artifact/u,
  );
});

test("BscTestnetSccp rejects native prover bundles that label executable artifacts as fixtures", () => {
  const fixture = sampleVerifiedBscTestnetNativeEvmProverFixture();

  assert.throws(
    () =>
      validateBscTestnetNativeEvmProverBundle(
        {
          ...fixture.bundle,
          proof_artifact: "artifacts/bsc-testnet/fixtures/proof-artifact.r1cs",
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /proofArtifact must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
  );

  assert.throws(
    () =>
      validateBscTestnetNativeEvmProverBundle(
        {
          ...fixture.bundle,
          verifier_key: "artifacts/bsc-testnet/diagnostic/verifier-key.bin",
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /verifierKey must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
  );

  assert.throws(
    () =>
      validateBscTestnetNativeEvmProverBundle(
        {
          ...fixture.bundle,
          native_sdk_artifacts: fixture.bundle.native_sdk_artifacts.map(
            (artifact) =>
              artifact.sdk === "javascript"
                ? {
                    ...artifact,
                    implementation_artifact:
                      "artifacts/bsc-testnet/mock/javascript-implementation.bin",
                  }
                : artifact,
          ),
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /nativeSdkArtifacts\[0\]\.implementationArtifact must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
  );
});

test("BscTestnetSccp rejects tiny native prover material even when hashes are self-consistent", () => {
  const fixture = sampleVerifiedBscTestnetNativeEvmProverFixture();
  const tinyProofArtifactBytes = nativeEvmProverArtifactBytes(
    "tiny bsc proof artifact",
    256,
  );
  const proofArtifactHash = sha256Hex(tinyProofArtifactBytes);
  const draftBundle = {
    ...fixture.bundle,
    proof_artifact_hash: proofArtifactHash,
    native_sdk_artifacts: fixture.bundle.native_sdk_artifacts.map((artifact) => ({
      ...artifact,
      prover_artifact_hash: proofArtifactHash,
    })),
  };
  const parityFixtureBytes =
    sampleBscTestnetNativeEvmProverParityFixtureBytes(draftBundle);
  const selfTestFixtureBytes =
    sampleBscTestnetNativeEvmProverSelfTestFixtureBytes(draftBundle);
  const bundle = {
    ...draftBundle,
    audit_hashes: {
      ...draftBundle.audit_hashes,
      cross_sdk_fixture_parity: sha256Hex(parityFixtureBytes),
      native_prover_self_test: sha256Hex(selfTestFixtureBytes),
    },
  };

  assert.throws(
    () =>
      verifyBscTestnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes: tinyProofArtifactBytes,
          provingKeyBytes: fixture.provingKeyBytes,
          verifierKeyBytes: fixture.verifierKeyBytes,
          crossSdkFixtureParityBytes: parityFixtureBytes,
          nativeProverSelfTestBytes: selfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes: fixture.implementationBytes,
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /proofArtifactBytes must be at least 65536 bytes/u,
  );
});

test("BscTestnetSccp rejects hash-consistent malformed native proof artifacts", () => {
  const fixture = sampleVerifiedBscTestnetNativeEvmProverFixture();
  const badProofArtifactBytes = Buffer.from(fixture.proofArtifactBytes);
  badProofArtifactBytes.writeUInt32LE(badProofArtifactBytes.length, 16);
  const proofArtifactHash = sha256Hex(badProofArtifactBytes);
  const draftBundle = {
    ...fixture.bundle,
    proof_artifact_hash: proofArtifactHash,
    native_sdk_artifacts: fixture.bundle.native_sdk_artifacts.map((artifact) => ({
      ...artifact,
      prover_artifact_hash: proofArtifactHash,
    })),
  };
  const parityFixtureBytes =
    sampleBscTestnetNativeEvmProverParityFixtureBytes(draftBundle);
  const selfTestFixtureBytes =
    sampleBscTestnetNativeEvmProverSelfTestFixtureBytes(draftBundle);
  const bundle = {
    ...draftBundle,
    audit_hashes: {
      ...draftBundle.audit_hashes,
      cross_sdk_fixture_parity: sha256Hex(parityFixtureBytes),
      native_prover_self_test: sha256Hex(selfTestFixtureBytes),
    },
  };

  assert.throws(
    () =>
      verifyBscTestnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes: badProofArtifactBytes,
          provingKeyBytes: fixture.provingKeyBytes,
          verifierKeyBytes: fixture.verifierKeyBytes,
          crossSdkFixtureParityBytes: parityFixtureBytes,
          nativeProverSelfTestBytes: selfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes: fixture.implementationBytes,
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /proofArtifactBytes \.r1cs section exceeds file size/u,
  );
});

test("BscTestnetSccp rejects hash-consistent malformed native proving keys", () => {
  const fixture = sampleVerifiedBscTestnetNativeEvmProverFixture();
  const badProvingKeyBytes = Buffer.from(fixture.provingKeyBytes);
  badProvingKeyBytes.writeUInt32LE(badProvingKeyBytes.length, 16);
  const provingKeyHash = sha256Hex(badProvingKeyBytes);
  const draftBundle = {
    ...fixture.bundle,
    proving_key_hash: provingKeyHash,
    native_sdk_artifacts: fixture.bundle.native_sdk_artifacts.map((artifact) => ({
      ...artifact,
      proving_key_hash: provingKeyHash,
    })),
  };
  const parityFixtureBytes =
    sampleBscTestnetNativeEvmProverParityFixtureBytes(draftBundle);
  const selfTestFixtureBytes =
    sampleBscTestnetNativeEvmProverSelfTestFixtureBytes(draftBundle);
  const bundle = {
    ...draftBundle,
    audit_hashes: {
      ...draftBundle.audit_hashes,
      cross_sdk_fixture_parity: sha256Hex(parityFixtureBytes),
      native_prover_self_test: sha256Hex(selfTestFixtureBytes),
    },
  };

  assert.throws(
    () =>
      verifyBscTestnetNativeEvmProverArtifacts(
        {
          nativeProverBundle: bundle,
          proofArtifactBytes: fixture.proofArtifactBytes,
          provingKeyBytes: badProvingKeyBytes,
          verifierKeyBytes: fixture.verifierKeyBytes,
          crossSdkFixtureParityBytes: parityFixtureBytes,
          nativeProverSelfTestBytes: selfTestFixtureBytes,
          sdk: "javascript",
          implementationBytes: fixture.implementationBytes,
        },
        { destinationBinding: fixture.destinationBinding },
      ),
    /provingKeyBytes \.zkey section exceeds file size/u,
  );
});

test("BscMainnetSccp validates native prover bundle manifests with mainnet literals", () => {
  const destinationBinding = bscMainnetSccpDestinationBinding(
    sampleDestinationBindingInput(),
  );
  const bundle = sampleBscTestnetNativeEvmProverBundle(
    destinationBinding.bindingHash,
    {
      bundle_id: SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
      chain: "bsc-mainnet",
    },
  );
  const descriptor = validateBscMainnetNativeEvmProverBundle(bundle, {
    destinationBinding,
  });
  assert.deepEqual(
    parseBscMainnetNativeEvmProverBundleManifest(JSON.stringify(bundle), {
      destinationBinding,
    }),
    descriptor,
  );
  assert.equal(
    descriptor.bundleId,
    SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
  );
  assert.equal(descriptor.chain, "bsc-mainnet");

  const parityFixture = sampleBscTestnetNativeEvmProverParityFixture(bundle, {
    schema: SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
    chain: "bsc-mainnet",
  });
  assert.deepEqual(
    parseBscMainnetNativeEvmProverParityFixture(
      JSON.stringify(parityFixture),
      bundle,
    ),
    validateBscMainnetNativeEvmProverParityFixture(parityFixture, bundle),
  );

  const selfTestFixture = sampleBscTestnetNativeEvmProverSelfTestFixture(
    bundle,
    {
      schema: SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
      chain: "bsc-mainnet",
    },
  );
  assert.deepEqual(
    parseBscMainnetNativeEvmProverSelfTestFixture(
      JSON.stringify(selfTestFixture),
      bundle,
    ),
    validateBscMainnetNativeEvmProverSelfTestFixture(selfTestFixture, bundle),
  );

  assert.throws(
    () =>
      validateBscMainnetNativeEvmProverBundle(
        {
          ...bundle,
          bundle_id: SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
        },
        { destinationBinding },
      ),
    /bundleId must be/u,
  );
});

test("BscMainnetSccp calldata requires a wrapped BSC mainnet proof result", () => {
  const { destinationBinding, nativeProverArtifacts } =
    sampleVerifiedBscMainnetNativeEvmProverFixture();
  const sdk = new BscMainnetSccp({ nativeProverArtifacts });
  const request = sdk.buildOutboundProofRequest(
    sampleOutboundInput(
      SCCP_DOMAIN_BSC,
      destinationBindingInputFromBinding(destinationBinding),
    ),
  );
  const proofResult = wrapBscMainnetSccpDestinationProofResult(
    groth16ProofBytes(request.publicInputs),
    request,
  );
  const submission = sdk.buildBscCalldata({ proofResult });

  assert.equal(submission.targetDomain, SCCP_DOMAIN_BSC);
  assert.equal(submission.destinationBindingHash, request.destinationBindingHash);

  assert.throws(
    () =>
      wrapBscMainnetSccpDestinationProofResult(
        groth16ProofBytes(request.publicInputs),
        {
        ...request,
        destinationBindingHash: hex32("99"),
        },
      ),
    /canonical|destinationBinding/u,
  );

  assert.throws(
    () =>
      sdk.buildBscCalldata({
        publicInputs: samplePublicInputs,
        proofBytes: groth16ProofBytes(request.publicInputs),
        sourceDomain: SCCP_DOMAIN_SORA,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
      }),
    /wrapped proofResult/u,
  );
  assert.throws(
    () =>
      sdk.buildBscCalldata({
        proofResult: {
          ...proofResult,
          destinationBinding: {
            ...proofResult.destinationBinding,
            networkId: hex32("33"),
          },
        },
      }),
    /chain id 56|destinationBinding/u,
  );
});

test("BscMainnetSccp binds custom outbound proof results to the requested proof", async () => {
  const { destinationBinding, nativeProverArtifacts } =
    sampleVerifiedBscMainnetNativeEvmProverFixture();
  const input = sampleOutboundInput(
    SCCP_DOMAIN_BSC,
    destinationBindingInputFromBinding(destinationBinding),
  );
  const nativeProverSelfTest = (context) => context.expectedResult;
  const referenceSdk = new BscMainnetSccp({ nativeProverArtifacts });
  const expectedRequest = referenceSdk.buildOutboundProofRequest(input);
  const wrongFixture = buildSampleOutboundBundleFixture({ nonce: 2n });
  const wrongRequest = referenceSdk.buildOutboundProofRequest({
    ...sampleOutboundInput(
      SCCP_DOMAIN_BSC,
      destinationBindingInputFromBinding(destinationBinding),
    ),
    publicInputs: wrongFixture.publicInputs,
    bundleBytes: wrongFixture.bundleBytes,
  });
  const wrongProofResult = {
    ...wrapBscMainnetSccpDestinationProofResult(
      groth16ProofBytes(expectedRequest.publicInputs),
      expectedRequest,
    ),
    requestHash: wrongRequest.requestHash,
  };
  let seenRequest;
  const rejectingSdk = new BscMainnetSccp({
    nativeProverArtifacts,
    nativeProverSelfTest,
    outboundProver: {
      async prove(request) {
        seenRequest = request;
        return wrongProofResult;
      },
    },
  });

  assert.notEqual(wrongRequest.requestHash, expectedRequest.requestHash);
  await assert.rejects(
    () => rejectingSdk.proveOutboundToBsc(input),
    /requestHash must match request/u,
  );
  assert.equal(seenRequest.requestHash, expectedRequest.requestHash);
  assert.equal(seenRequest.targetDomain, SCCP_DOMAIN_BSC);
  assert.equal(Object.isFrozen(seenRequest), true);

  const zeroProofBytes = new Uint8Array(GROTH16_PROOF_BYTES.length);
  assert.throws(
    () => wrapBscMainnetSccpDestinationProofResult(zeroProofBytes, expectedRequest),
    /proofBytes must not be all zero/u,
  );
  const zeroProofSdk = new BscMainnetSccp({
    nativeProverArtifacts,
    nativeProverSelfTest,
    outboundProver: {
      async prove() {
        return { proofBytes: zeroProofBytes };
      },
    },
  });
  await assert.rejects(
    () => zeroProofSdk.proveOutboundToBsc(input),
    /proofBytes must not be all zero/u,
  );

  let acceptedRequest;
  const acceptingSdk = new BscMainnetSccp({
    nativeProverArtifacts,
    nativeProverSelfTest,
    outboundProver: {
      async prove(request) {
        acceptedRequest = request;
        return wrapBscMainnetSccpDestinationProofResult(
          groth16ProofBytes(request.publicInputs),
          request,
        );
      },
    },
  });
  const proofResult = await acceptingSdk.proveOutboundToBsc(input);
  assert.equal(acceptedRequest.requestHash, expectedRequest.requestHash);
  assert.equal(proofResult.requestHash, expectedRequest.requestHash);
});

test("BscMainnetSccp outbound provider path derives target from wrapped proof result", async () => {
  const { destinationBinding, nativeProverArtifacts } =
    sampleVerifiedBscMainnetNativeEvmProverFixture();
  const submittedTxs = [];
  const provider = {
    async request({ method, params }) {
      if (method === "eth_chainId") return "0x38";
      if (method === "eth_sendTransaction") {
        submittedTxs.push(params[0]);
        return `0xbsc${submittedTxs.length}`;
      }
      throw new Error(`unexpected RPC method ${method}`);
    },
  };
  const sdk = new BscMainnetSccp({
    executionProvider: provider,
    nativeProverArtifacts,
  });
  const request = sdk.buildOutboundProofRequest(
    sampleOutboundInput(
      SCCP_DOMAIN_BSC,
      destinationBindingInputFromBinding(destinationBinding),
    ),
  );
  const proofResult = wrapBscMainnetSccpDestinationProofResult(
    groth16ProofBytes(request.publicInputs),
    request,
  );

  assert.equal(await sdk.submitOutboundToBsc({ proofResult }), "0xbsc1");
  assert.equal(submittedTxs[0].to, request.destinationBinding.bridgeAddress);
  assert.equal(submittedTxs[0].data, sdk.buildBscCalldata({ proofResult }).callDataHex);

  const { destinationBinding: proofDestinationBinding, ...proofResultWithoutBinding } = proofResult;
  const { bridgeAddress: _bridgeAddress, ...bindingWithoutBridge } = proofDestinationBinding;
  const snakeProofResult = {
    ...proofResultWithoutBinding,
    destination_binding: {
      ...bindingWithoutBridge,
      bridge_address: proofDestinationBinding.bridgeAddress,
    },
  };
  assert.equal(await sdk.submitOutboundToBsc({ proof_result: snakeProofResult }), "0xbsc2");
  assert.equal(submittedTxs[1].to, request.destinationBinding.bridgeAddress);

  assert.equal(
    await sdk.submitOutboundToBsc({
      proofResult,
      to: request.destinationBinding.bridgeAddress.toUpperCase(),
    }),
    "0xbsc3",
  );
  assert.equal(submittedTxs[2].to, request.destinationBinding.bridgeAddress);

  await assert.rejects(
    () => sdk.submitOutboundToBsc({ proofResult, to: `0x${"77".repeat(20)}` }),
    /to address must match proofResult\.destinationBinding\.bridgeAddress/u,
  );
  assert.equal(submittedTxs.length, 3);
});

test("BscMainnetSccp validates configured providers before app-owned submit callbacks", async () => {
  const { destinationBinding, nativeProverArtifacts } =
    sampleVerifiedBscMainnetNativeEvmProverFixture();
  const request = new BscMainnetSccp({
    nativeProverArtifacts,
  }).buildOutboundProofRequest(
    sampleOutboundInput(
      SCCP_DOMAIN_BSC,
      destinationBindingInputFromBinding(destinationBinding),
    ),
  );
  const proofResult = wrapBscMainnetSccpDestinationProofResult(
    groth16ProofBytes(request.publicInputs),
    request,
  );
  let called = false;
  await assert.rejects(
    () =>
      new BscMainnetSccp({
        nativeProverArtifacts,
        executionProvider: {
          async request({ method }) {
            assert.equal(method, "eth_chainId");
            return "0x61";
          },
        },
        submitOutboundToBsc() {
          called = true;
          return "submitted";
        },
      }).submitOutboundToBsc({ proofResult }),
    /eth_chainId == 0x38/u,
  );
  assert.equal(called, false);

  const methods = [];
  const sdk = new BscMainnetSccp({
    nativeProverArtifacts,
    executionProvider: {
      async request({ method }) {
        methods.push(method);
        if (method === "eth_chainId") return "0x38";
        throw new Error(`unexpected RPC method ${method}`);
      },
    },
    submitOutboundToBsc(submission) {
      called = true;
      assert.equal(submission.destinationBindingHash, request.destinationBindingHash);
      return "submitted";
    },
  });
  assert.equal(await sdk.submitOutboundToBsc({ proofResult }), "submitted");
  assert.equal(called, true);
  assert.deepEqual(methods, ["eth_chainId"]);

  let callbackOnlyCalled = false;
  assert.equal(
    await new BscMainnetSccp({
      nativeProverArtifacts,
      submitOutboundToBsc(submission) {
        callbackOnlyCalled = true;
        return submission.targetDomain;
      },
    }).submitOutboundToBsc({ proofResult }),
    SCCP_DOMAIN_BSC,
  );
  assert.equal(callbackOnlyCalled, true);
});

test("BscTestnetSccp outbound provider path submits on chain id 0x61", async () => {
  const { destinationBinding, nativeProverArtifacts } =
    sampleVerifiedBscTestnetNativeEvmProverFixture();
  const submittedTxs = [];
  const provider = {
    async request({ method, params }) {
      if (method === "eth_chainId") return "0x61";
      if (method === "eth_sendTransaction") {
        submittedTxs.push(params[0]);
        return `0xbscTestnet${submittedTxs.length}`;
      }
      throw new Error(`unexpected RPC method ${method}`);
    },
  };
  const sdk = new BscTestnetSccp({
    executionProvider: provider,
    nativeProverArtifacts,
  });
  const request = sdk.buildOutboundProofRequest(
    sampleBscTestnetOutboundInput(
      SCCP_DOMAIN_BSC,
      destinationBindingInputFromBinding(destinationBinding),
    ),
  );
  const proofResult = wrapBscTestnetSccpDestinationProofResult(
    groth16ProofBytes(request.publicInputs),
    request,
  );

  assert.equal(await sdk.submitOutboundToBsc({ proofResult }), "0xbscTestnet1");
  assert.equal(submittedTxs[0].to, request.destinationBinding.bridgeAddress);
  assert.equal(submittedTxs[0].data, sdk.buildBscCalldata({ proofResult }).callDataHex);
  assert.equal(submittedTxs[0].chainId, "0x61");

  await assert.rejects(
    () =>
      new BscTestnetSccp({
        executionProvider: {
          async request({ method }) {
            if (method === "eth_chainId") return "0x38";
            throw new Error(`unexpected RPC method ${method}`);
          },
        },
        nativeProverArtifacts,
      }).submitOutboundToBsc({ proofResult }),
    /eth_chainId == 0x61/u,
  );
});

test("BscTestnetSccp validates configured providers before app-owned submit callbacks", async () => {
  const { destinationBinding, nativeProverArtifacts } =
    sampleVerifiedBscTestnetNativeEvmProverFixture();
  const request = new BscTestnetSccp({ nativeProverArtifacts }).buildOutboundProofRequest(
    sampleBscTestnetOutboundInput(
      SCCP_DOMAIN_BSC,
      destinationBindingInputFromBinding(destinationBinding),
    ),
  );
  const proofResult = wrapBscTestnetSccpDestinationProofResult(
    groth16ProofBytes(request.publicInputs),
    request,
  );
  let called = false;
  await assert.rejects(
    () =>
      new BscTestnetSccp({
        executionProvider: {
          async request({ method }) {
            assert.equal(method, "eth_chainId");
            return "0x38";
          },
        },
        submitOutboundToBsc() {
          called = true;
          return "submitted";
        },
        nativeProverArtifacts,
      }).submitOutboundToBsc({ proofResult }),
    /eth_chainId == 0x61/u,
  );
  assert.equal(called, false);

  const methods = [];
  assert.equal(
    await new BscTestnetSccp({
      executionProvider: {
        async request({ method }) {
          methods.push(method);
          if (method === "eth_chainId") return "0x61";
          throw new Error(`unexpected RPC method ${method}`);
        },
      },
      submitOutboundToBsc(submission) {
        called = true;
        assert.equal(submission.destinationBindingHash, request.destinationBindingHash);
        return "submitted";
      },
      nativeProverArtifacts,
    }).submitOutboundToBsc({ proofResult }),
    "submitted",
  );
  assert.equal(called, true);
  assert.deepEqual(methods, ["eth_chainId"]);
});

test("BscMainnetSccp builds BSC -> SORA local-admission submissions", () => {
  const input = {
    sourceDomain: SCCP_DOMAIN_BSC,
    targetDomain: SCCP_DOMAIN_SORA,
    proofBytes: [1, 2, 3],
    publicInputsBytes: [4, 5, 6],
    bundleBytes: [7, 8, 9],
    envelopeBytes: [10, 11, 12],
    statementHash: hex32("66"),
    sourceVerifierMaterialHash: hex32("77"),
    sourceAdapterEngineDeploymentHash: hex32("88"),
  };
  const submission = buildBscMainnetSccpLocalAdmissionSubmission(input);
  const facadeSubmission = new BscMainnetSccp().buildLocalAdmissionSubmission(input);
  const testnetSubmission = buildBscTestnetSccpLocalAdmissionSubmission(input);
  const testnetFacadeSubmission = new BscTestnetSccp().buildLocalAdmissionSubmission(input);

  assert.equal(submission.platformPayload, SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1);
  assert.equal(submission.envelopeEncoding, SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1);
  assert.equal(submission.verifierEntrypoint, SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1);
  assert.equal(submission.sourceDomain, SCCP_DOMAIN_BSC);
  assert.equal(submission.targetDomain, SCCP_DOMAIN_SORA);
  assert.deepEqual([...submission.arguments], []);
  assert.deepEqual([...submission.proofBytes], [1, 2, 3]);
  assert.deepEqual([...submission.publicInputsBytes], [4, 5, 6]);
  assert.deepEqual([...submission.bundleBytes], [7, 8, 9]);
  assert.deepEqual([...submission.envelopeBytes], [10, 11, 12]);
  assert.deepEqual([...submission.localAdmission.proofBytes], [1, 2, 3]);
  assert.equal(facadeSubmission.envelopeHex, submission.envelopeHex);
  assert.equal(testnetSubmission.sourceDomain, SCCP_DOMAIN_BSC);
  assert.equal(testnetSubmission.targetDomain, SCCP_DOMAIN_SORA);
  assert.equal(testnetFacadeSubmission.envelopeHex, testnetSubmission.envelopeHex);

  input.proofBytes[0] = 99;
  assert.deepEqual([...submission.proofBytes], [1, 2, 3]);
  assert.deepEqual([...testnetSubmission.proofBytes], [1, 2, 3]);

  assert.throws(
    () => buildBscMainnetSccpLocalAdmissionSubmission({ ...input, sourceDomain: SCCP_DOMAIN_ETH }),
    /BSC -> SORA/u,
  );
  assert.throws(
    () => buildBscMainnetSccpLocalAdmissionSubmission({ ...input, proofBytes: [0, 0] }),
    /proofBytes must not be all zero/u,
  );
  assert.throws(
    () => buildBscMainnetSccpLocalAdmissionSubmission({ ...input, envelopeBytes: [] }),
    /envelopeBytes must not be empty/u,
  );
  assert.throws(
    () =>
      buildBscMainnetSccpLocalAdmissionSubmission({
        ...input,
        envelopeEncoding: "abi_tuple_v1",
      }),
    /metadata is not canonical/u,
  );
  assert.throws(
    () =>
      buildBscMainnetSccpLocalAdmissionSubmission({
        ...input,
        proofFamily: "debug-proof-family",
      }),
    /metadata is not canonical/u,
  );
});

test("BscMainnetSccp inbound proving rejects foreign EVM domains before callbacks run", async () => {
  let called = false;
  const sdk = new BscMainnetSccp({
    proveInbound() {
      called = true;
    },
  });

  await assert.rejects(
    () =>
      sdk.proveInboundToSora({
        sourceDomain: SCCP_DOMAIN_ETH,
        targetDomain: SCCP_DOMAIN_SORA,
      }),
    /sourceDomain must be BSC/u,
  );
  assert.equal(called, false);
});

test("BscMainnetSccp requires full receipt proof evidence before inbound proving", async () => {
  const receiptProofHash = hex32("ee");
  const evidence = await new BscMainnetSccp().collectInboundEvidenceFromReceipt({
    receiptProofHash,
    parliaFinality: sampleParliaFinality(),
  });

  assert.equal(evidence.sourceDomain, SCCP_DOMAIN_BSC);
  assert.equal(evidence.targetDomain, SCCP_DOMAIN_SORA);
  assert.equal(evidence.receiptProofHash, receiptProofHash);
  assert.equal(evidence.parliaFinality.executionBlockNumber, "4660");
  assert.equal(evidence.receipt, undefined);
  assert.equal(evidence.block, undefined);

  let calledWithHashOnly = false;
  await assert.rejects(
    () =>
      new BscMainnetSccp({
        proveInbound() {
          calledWithHashOnly = true;
          return [7, 8, 9];
        },
      }).proveInboundToSora({
        receipt_proof_hash: receiptProofHash,
        finalityEvidence: sampleParliaFinality(),
      }),
    /requires receiptProof/u,
  );
  assert.equal(calledWithHashOnly, false);

  const fullProofHash = bscSccpReceiptProofHash(sampleReceiptProof);
  let callbackEvidence;
  assert.deepEqual(
    [
      ...(await new BscMainnetSccp({
        proveInbound(proverEvidence) {
          callbackEvidence = proverEvidence;
          return [7, 8, 9];
        },
      }).proveInboundToSora({
        ...sampleInboundEvidence(),
        receiptProof: sampleReceiptProof,
        receipt_proof_hash: fullProofHash,
      })),
    ],
    [7, 8, 9],
  );
  assert.equal(callbackEvidence.receiptProofHash, fullProofHash);
  assert.equal(callbackEvidence.receiptProof.blockHash, BLOCK_HASH);
  assert.equal(callbackEvidence.sourceEventDigest, SOURCE_EVENT_DIGEST);
  assert.equal(callbackEvidence.sourceBridgeEmitterAddress, SOURCE_BRIDGE_ADDRESS);

  const fullProofEvidence = await new BscMainnetSccp().collectInboundEvidenceFromReceipt({
    ...sampleInboundEvidence(),
    receiptProof: sampleReceiptProof,
    receiptProofHash: fullProofHash,
    parliaFinality: sampleParliaFinality(),
  });
  assert.equal(fullProofEvidence.receiptProofHash, fullProofHash);
  assert.equal(fullProofEvidence.sourceEventDigest, SOURCE_EVENT_DIGEST);

  let calledWithoutSourceEvent = false;
  await assert.rejects(
    () =>
      new BscMainnetSccp({
        proveInbound() {
          calledWithoutSourceEvent = true;
          return [7, 8, 9];
        },
      }).proveInboundToSora({
        receiptProof: sampleReceiptProof,
        receiptProofHash: fullProofHash,
        parliaFinality: sampleParliaFinality(),
      }),
    /requires receipt source event validation/u,
  );
  assert.equal(calledWithoutSourceEvent, false);

  await assert.rejects(
    () =>
      new BscMainnetSccp().collectInboundEvidenceFromReceipt({
        ...sampleInboundEvidence(),
        receipt: {
          ...sampleInboundEvidence().receipt,
          logs: [sourceEventLog({ topics: [evmSccpSourceEventTopic(), hex32("35")] })],
        },
        receiptProof: sampleReceiptProof,
        receiptProofHash: fullProofHash,
        parliaFinality: sampleParliaFinality(),
      }),
    /receiptProof\.sourceEventDigest must match receipt source event/u,
  );

  const malformedSourceLogCases = [
    [
      "extra topic",
      [sourceEventLog({ topics: [evmSccpSourceEventTopic(), SOURCE_EVENT_DIGEST, hex32("66")] })],
      /SCCP source event log must contain exactly 2 topics/u,
    ],
    [
      "non-empty data",
      [sourceEventLog({ data: "0x01" })],
      /SCCP source event log data must be 0x/u,
    ],
    [
      "zero digest",
      [sourceEventLog({ topics: [evmSccpSourceEventTopic(), hex32("00")] })],
      /SCCP source event digest must not be zero/u,
    ],
    [
      "duplicate events",
      [sourceEventLog(), sourceEventLog()],
      /exactly one matching SCCP source event/u,
    ],
    [
      "removed event",
      [sourceEventLog({ removed: true })],
      /removed logs/u,
    ],
  ];
  for (const [label, logs, message] of malformedSourceLogCases) {
    await assert.rejects(
      () =>
        new BscMainnetSccp().collectInboundEvidenceFromReceipt({
          ...sampleInboundEvidence(),
          receipt: {
            ...sampleInboundEvidence().receipt,
            logs,
          },
        }),
      message,
      label,
    );
  }

  const missingTransactionHashLog = sourceEventLog();
  delete missingTransactionHashLog.transactionHash;
  await assert.rejects(
    () =>
      new BscMainnetSccp().collectInboundEvidenceFromReceipt({
        ...sampleInboundEvidence(),
        receipt: {
          ...sampleInboundEvidence().receipt,
          logs: [missingTransactionHashLog],
        },
      }),
    /receipt\.logs\[0\]\.transactionHash/u,
  );

  await assert.rejects(
    () =>
      new BscMainnetSccp().collectInboundEvidenceFromReceipt({
        receiptProof: sampleReceiptProof,
        receiptProofHash: hex32("99"),
        parliaFinality: sampleParliaFinality(),
      }),
    /receiptProofHash must match receiptProof/u,
  );
  await assert.rejects(
    () =>
      new BscMainnetSccp().collectInboundEvidenceFromReceipt({
        receiptProofHash: hex32("00"),
        parliaFinality: sampleParliaFinality(),
      }),
    /receiptProofHash must not be zero/u,
  );
  await assert.rejects(
    () =>
      new BscMainnetSccp().collectInboundEvidenceFromReceipt({
        receiptProofHash: "0x123",
        parliaFinality: sampleParliaFinality(),
      }),
    /receiptProofHash must be canonical hex/u,
  );
  await assert.rejects(
    () =>
      new BscMainnetSccp().collectInboundEvidenceFromReceipt({
        receiptProofHash,
        receipt_proof_hash: receiptProofHash,
        parliaFinality: sampleParliaFinality(),
      }),
    /receiptProofHash must not use multiple aliases/u,
  );
});

test("BscTestnetSccp proves inbound BSC-family receipt evidence", async () => {
  const fullProofHash = bscSccpReceiptProofHash(sampleReceiptProof);
  let callbackEvidence;
  const sdk = new BscTestnetSccp({
    proveInbound(proverEvidence) {
      callbackEvidence = proverEvidence;
      return [9, 8, 7];
    },
  });

  assert.deepEqual(
    [
      ...(await sdk.proveInboundToSora({
        ...sampleInboundEvidence(),
        receiptProof: sampleReceiptProof,
        receiptProofHash: fullProofHash,
      })),
    ],
    [9, 8, 7],
  );
  assert.equal(callbackEvidence.sourceDomain, SCCP_DOMAIN_BSC);
  assert.equal(callbackEvidence.targetDomain, SCCP_DOMAIN_SORA);
  assert.equal(callbackEvidence.receiptProofHash, fullProofHash);
  assert.equal(callbackEvidence.sourceEventDigest, SOURCE_EVENT_DIGEST);
});

test("BscMainnetSccp inbound proving requires Parlia finality before callbacks run", async () => {
  let called = false;
  const sdk = new BscMainnetSccp({
    proveInbound() {
      called = true;
      return Uint8Array.from([1]);
    },
  });

  const { parliaFinality: _parliaFinality, ...withoutFinality } = sampleInboundEvidence();
  await assert.rejects(
    () => sdk.proveInboundToSora(withoutFinality),
    /requires parliaFinality/u,
  );
  await assert.rejects(
    () => sdk.proveInboundToSora({ ...sampleInboundEvidence(), parliaFinality: {} }),
    /parliaFinality\.executionBlockNumber/u,
  );
  assert.equal(called, false);
});

test("BscMainnetSccp rejects Parlia finality drift from the execution block", async () => {
  const sdk = new BscMainnetSccp();

  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        ...sampleInboundEvidence(),
        parliaFinality: sampleParliaFinality({ executionBlockHash: hex32("bc") }),
      }),
    /parliaFinality\.executionBlockHash/u,
  );
  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        ...sampleInboundEvidence(),
        parliaFinality: sampleParliaFinality({ executionBlockNumber: "0x1235" }),
      }),
    /parliaFinality\.executionBlockNumber/u,
  );
  await assert.rejects(
    () =>
      sdk.collectInboundEvidenceFromReceipt({
        ...sampleInboundEvidence(),
        parliaFinality: sampleParliaFinality({ executionReceiptsRoot: hex32("cd") }),
      }),
    /parliaFinality\.executionReceiptsRoot/u,
  );
});

test("BscMainnetSccp inbound proving rejects empty or all-zero proof output", async () => {
  let callbackEvidence;
  const sdk = new BscMainnetSccp({
    proveInbound(evidence) {
      callbackEvidence = evidence;
      return Uint8Array.from([7, 8, 9]);
    },
  });
  const inboundEvidence = { ...sampleInboundEvidence(), receiptProof: sampleReceiptProof };

  assert.deepEqual(
    [...(await sdk.proveInboundToSora(inboundEvidence))],
    [7, 8, 9],
  );
  assert.equal(callbackEvidence.receipt.blockHash, BLOCK_HASH);
  assert.equal(callbackEvidence.receiptProof.blockHash, BLOCK_HASH);
  assert.equal(callbackEvidence.parliaFinality.commitSealHash, hex32("dd"));

  await assert.rejects(
    () =>
      new BscMainnetSccp({
        proveInbound() {
          return new Uint8Array();
        },
      }).proveInboundToSora(inboundEvidence),
    /proofBytes must not be empty/u,
  );
  await assert.rejects(
    () =>
      new BscMainnetSccp({
        proveInbound() {
          return Uint8Array.from([0, 0]);
        },
      }).proveInboundToSora(inboundEvidence),
    /proofBytes must not be all zero/u,
  );
});

test("BscMainnetSccp inbound submitter receives copied non-zero proof bytes", async () => {
  let submitted;
  const sdk = new BscMainnetSccp({
    submitInboundToIroha(proofBytes) {
      submitted = proofBytes;
      return "submitted";
    },
  });
  const proofBytes = Uint8Array.from([1, 2, 3]);

  assert.equal(await sdk.submitInboundToIroha(proofBytes), "submitted");
  proofBytes[0] = 9;
  assert.deepEqual([...submitted], [1, 2, 3]);

  await assert.rejects(
    () => sdk.submitInboundToIroha(new Uint8Array()),
    /proofBytes must not be empty/u,
  );
  await assert.rejects(
    () => sdk.submitInboundToIroha(Uint8Array.from([0, 0, 0])),
    /proofBytes must not be all zero/u,
  );
});
