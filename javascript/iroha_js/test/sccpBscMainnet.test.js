import { test } from "node:test";
import assert from "node:assert/strict";
import {
  BscMainnetSccp,
  SCCP_BSC_MAINNET_EVM_CHAIN_ID,
  SCCP_BSC_MAINNET_NETWORK_ID,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_ETH,
  SCCP_DOMAIN_SORA,
  SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1,
  SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
  SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1,
  SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1,
  bscMainnetSccpDestinationBinding,
  bscSccpReceiptProofHash,
  buildBscMainnetSccpLocalAdmissionSubmission,
  wrapBscMainnetSccpDestinationProofResult,
} from "../src/sccp.js";

const hex32 = (byte) => `0x${byte.repeat(32)}`;
const TX_HASH = hex32("aa");
const BLOCK_HASH = hex32("bb");
const RECEIPTS_ROOT = hex32("cc");

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
  receipt: {
    transactionHash: TX_HASH,
    blockHash: BLOCK_HASH,
    blockNumber: "0x1234",
    status: "0x1",
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
  sourceEventDigest: hex32("34"),
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

const samplePublicInputs = {
  messageId: hex32("11"),
  payloadHash: hex32("22"),
  targetDomain: SCCP_DOMAIN_BSC,
  commitmentRoot: hex32("33"),
  finalityHeight: "42",
  finalityBlockHash: hex32("55"),
};

const sampleDestinationBindingInput = (overrides = {}) => ({
  verifierAddress: `0x${"11".repeat(20)}`,
  bridgeAddress: `0x${"22".repeat(20)}`,
  verifierCodeHash: hex32("bb"),
  verifierKeyHash: hex32("cc"),
  ...overrides,
});

const sampleOutboundInput = (targetDomain = SCCP_DOMAIN_BSC, destinationBindingOverrides = {}) => ({
  publicInputs: { ...samplePublicInputs, targetDomain },
  bundleBytes: [1, 2, 3],
  destinationBinding: bscMainnetSccpDestinationBinding(
    sampleDestinationBindingInput(destinationBindingOverrides),
  ),
  sourceDomain: SCCP_DOMAIN_SORA,
  statementHash: hex32("66"),
});

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

const groth16ProofBytes = () => {
  const out = new Uint8Array(SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1);
  const words = [
    abiWord(1),
    Uint8Array.from({ length: 32 }, () => 0x11),
    abiWord(SCCP_DOMAIN_SORA),
    Uint8Array.from({ length: 32 }, () => 0x33),
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

test("BscMainnetSccp rejects Ethereum and padded JSON-RPC chain ids", async () => {
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
    /hex, decimal, number, or bigint chain id/u,
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

test("BscMainnetSccp calldata requires a wrapped BSC mainnet proof result", () => {
  const sdk = new BscMainnetSccp();
  const request = sdk.buildOutboundProofRequest(sampleOutboundInput());
  const proofResult = wrapBscMainnetSccpDestinationProofResult(GROTH16_PROOF_BYTES, request);
  const submission = sdk.buildBscCalldata({ proofResult });

  assert.equal(submission.targetDomain, SCCP_DOMAIN_BSC);
  assert.equal(submission.destinationBindingHash, request.destinationBindingHash);

  assert.throws(
    () =>
      sdk.buildBscCalldata({
        publicInputs: samplePublicInputs,
        proofBytes: GROTH16_PROOF_BYTES,
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

test("BscMainnetSccp outbound provider path derives target from wrapped proof result", async () => {
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
  const sdk = new BscMainnetSccp({ executionProvider: provider });
  const request = sdk.buildOutboundProofRequest(sampleOutboundInput());
  const proofResult = wrapBscMainnetSccpDestinationProofResult(GROTH16_PROOF_BYTES, request);

  assert.equal(await sdk.submitOutboundToBsc({ proofResult }), "0xbsc1");
  assert.equal(submittedTxs[0].to, request.destinationBinding.bridgeAddress);
  assert.equal(submittedTxs[0].data, sdk.buildBscCalldata({ proofResult }).callDataHex);

  const { destinationBinding, ...proofResultWithoutBinding } = proofResult;
  const { bridgeAddress: _bridgeAddress, ...bindingWithoutBridge } = destinationBinding;
  const snakeProofResult = {
    ...proofResultWithoutBinding,
    destination_binding: {
      ...bindingWithoutBridge,
      bridge_address: destinationBinding.bridgeAddress,
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

  input.proofBytes[0] = 99;
  assert.deepEqual([...submission.proofBytes], [1, 2, 3]);

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

test("BscMainnetSccp accepts hash-only receipt proof evidence", async () => {
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

  let callbackEvidence;
  assert.deepEqual(
    [
      ...(await new BscMainnetSccp({
        proveInbound(proverEvidence) {
          callbackEvidence = proverEvidence;
          return [7, 8, 9];
        },
      }).proveInboundToSora({
        receipt_proof_hash: receiptProofHash,
        finalityEvidence: sampleParliaFinality(),
      })),
    ],
    [7, 8, 9],
  );
  assert.equal(callbackEvidence.receiptProofHash, receiptProofHash);

  const fullProofHash = bscSccpReceiptProofHash(sampleReceiptProof);
  const fullProofEvidence = await new BscMainnetSccp().collectInboundEvidenceFromReceipt({
    receiptProof: sampleReceiptProof,
    receiptProofHash: fullProofHash,
    parliaFinality: sampleParliaFinality(),
  });
  assert.equal(fullProofEvidence.receiptProofHash, fullProofHash);

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

  assert.deepEqual(
    [...(await sdk.proveInboundToSora(sampleInboundEvidence()))],
    [7, 8, 9],
  );
  assert.equal(callbackEvidence.receipt.blockHash, BLOCK_HASH);
  assert.equal(callbackEvidence.parliaFinality.commitSealHash, hex32("dd"));

  await assert.rejects(
    () =>
      new BscMainnetSccp({
        proveInbound() {
          return new Uint8Array();
        },
      }).proveInboundToSora(sampleInboundEvidence()),
    /proofBytes must not be empty/u,
  );
  await assert.rejects(
    () =>
      new BscMainnetSccp({
        proveInbound() {
          return Uint8Array.from([0, 0]);
        },
      }).proveInboundToSora(sampleInboundEvidence()),
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
