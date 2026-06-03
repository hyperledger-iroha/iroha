import { test } from "node:test";
import assert from "node:assert/strict";
import {
  EthereumMainnetBeaconRestConsensusProvider,
  EthereumMainnetSccp,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_ETH,
  SCCP_DOMAIN_SORA,
  SCCP_ETH_MAINNET_EVM_CHAIN_ID,
  SCCP_ETH_MAINNET_NETWORK_ID,
  SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1,
  SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
  SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1,
  SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1,
  buildEvmReceiptTrieProofFromReceipts,
  buildEthereumMainnetSccpLocalAdmissionSubmission,
  canonicalEvmReceiptRlp,
  canonicalEvmSccpReceiptProofBytes,
  ethereumMainnetSccpDestinationBinding,
  evmReceiptTrieKey,
  evmSccpReceiptProofHash,
  evmSccpSourceEventTopic,
  wrapEvmSccpProofResult,
} from "../src/sccp.js";

const hex32 = (byte) => `0x${byte.repeat(32)}`;
const TX_HASH = hex32("aa");
const BLOCK_HASH = hex32("bb");
const SOURCE_EVENT_DIGEST = hex32("34");
const SOURCE_BRIDGE_ADDRESS = `0x${"44".repeat(20)}`;
const sampleReceiptProof = {
  sourceDomain: SCCP_DOMAIN_ETH,
  sourceEventDigest: SOURCE_EVENT_DIGEST,
  beaconSlot: "64",
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

const samplePublicInputs = {
  messageId: hex32("11"),
  payloadHash: hex32("22"),
  targetDomain: SCCP_DOMAIN_ETH,
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

const sampleOutboundInput = (targetDomain = SCCP_DOMAIN_ETH, destinationBindingOverrides = {}) => ({
  publicInputs: { ...samplePublicInputs, targetDomain },
  bundleBytes: [1, 2, 3],
  destinationBinding: ethereumMainnetSccpDestinationBinding(
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
  assert.deepEqual(
    calls.map(([method]) => method),
    [
      "eth_chainId",
      "eth_getTransactionReceipt",
      "eth_getBlockByHash",
      "eth_getBlockReceipts",
    ],
  );

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
    if (url === "https://beacon.example/eth/v1/beacon/headers/finalized") {
      return {
        ok: true,
        async json() {
          return {
            execution_optimistic: false,
            finalized: true,
            data: {
              root: hex32("dd"),
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
    if (url === "https://beacon.example/eth/v1/beacon/blocks/finalized/root") {
      return {
        ok: true,
        async json() {
          return {
            execution_optimistic: false,
            finalized: true,
            data: { root: hex32("dd") },
          };
        },
      };
    }
    if (url === "https://beacon.example/eth/v2/beacon/blocks/finalized") {
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
              finalized: { root: hex32("dd"), epoch: "2" },
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
        return { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") };
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
  assert.equal(evidence.beaconFinality.finalizedHeaderRoot, hex32("dd"));
  assert.equal(evidence.beaconFinality.syncCommitteeRoot, hex32("ee"));
  assert.equal(evidence.beaconFinality.beaconSlot, "64");
  assert.deepEqual(
    fetchCalls.map(([url]) => url),
    [
      "https://beacon.example/eth/v1/beacon/headers/finalized",
      "https://beacon.example/eth/v1/beacon/blocks/finalized/root",
      "https://beacon.example/eth/v2/beacon/blocks/finalized",
      "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints",
    ],
  );
  for (const [, init] of fetchCalls) {
    assert.equal(init.method, "GET");
    assert.equal(init.headers.authorization, "Bearer local");
  }
});

test("EthereumMainnetBeaconRestConsensusProvider rejects unsafe or incomplete Beacon REST data", async () => {
  const block = { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") };
  const validHeader = () => ({
    execution_optimistic: false,
    finalized: true,
    data: {
      root: hex32("dd"),
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
    data: { finalized: { root: hex32("dd"), epoch: "2" } },
  });
  const validBlockRoot = () => ({
    execution_optimistic: false,
    finalized: true,
    data: { root: hex32("dd") },
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
  const syncCommitteePayload = {
    syncCommitteePublicKeys: [`0x${"11".repeat(48)}`],
    syncCommitteeWeights: ["1"],
    syncCommitteePops: [`0x${"22".repeat(96)}`],
  };
  const providerFor = (
    headerResponse,
    checkpointResponse = validCheckpoint(),
    extra = {},
    blockResponse = { ok: true, json: async () => validBlock() },
    blockRootResponse = { ok: true, json: async () => validBlockRoot() },
  ) =>
    new EthereumMainnetBeaconRestConsensusProvider({
      endpoint: "https://beacon.example",
      syncCommitteeRoot: hex32("ee"),
      fetch: async (url) => {
        if (url.endsWith("/eth/v1/beacon/headers/finalized")) return headerResponse;
        if (url.endsWith("/eth/v1/beacon/blocks/finalized/root")) return blockRootResponse;
        if (url.endsWith("/eth/v2/beacon/blocks/finalized")) return blockResponse;
        if (url.endsWith("/eth/v1/beacon/states/finalized/finality_checkpoints")) {
          return checkpointResponse;
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
  assert.equal(unchecked.finalizedHeaderRoot, hex32("dd"));

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
  assert.equal(textEvidence.finalizedHeaderRoot, hex32("dd"));

  const streamEvidence = await providerFor(
    streamResponse([Buffer.from(JSON.stringify(validHeader()))]),
    streamResponse([Buffer.from(JSON.stringify(validCheckpoint()))]),
  ).collectFinalityEvidence({ block });
  assert.equal(streamEvidence.finalizedHeaderRoot, hex32("dd"));

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
          executionBlockNumber: "0x1234",
          executionBlockHash: BLOCK_HASH,
          executionReceiptsRoot: hex32("cc"),
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
          finalizedHeaderRoot: hex32("99"),
          syncCommitteeRoot: hex32("ee"),
          beaconSlot: "0x40",
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
        return wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, request);
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
  const sdk = new EthereumMainnetSccp({
    executionProvider: {
      async request() {
        executionRequests += 1;
        throw new Error("unexpected execution-provider fallback");
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
      assert.equal(error.code, "ERR_SCCP_ETH_OUTBOUND_PROVER_UNAVAILABLE");
      assert.match(error.message, /local JS\/native EVM prover/u);
      return true;
    },
  );
  assert.equal(executionRequests, 0);
});

test("EthereumMainnetSccp calldata requires a wrapped Ethereum mainnet proof result", () => {
  const sdk = new EthereumMainnetSccp();
  const request = sdk.buildOutboundProofRequest(sampleOutboundInput());
  const proofResult = wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, request);
  const submission = sdk.buildEthereumCalldata({ proofResult });

  assert.equal(submission.targetDomain, SCCP_DOMAIN_ETH);
  assert.equal(submission.destinationBindingHash, request.destinationBindingHash);

  assert.throws(
    () =>
      sdk.buildEthereumCalldata({
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

test("EthereumMainnetSccp binds custom outbound proof results to the requested proof", async () => {
  const input = sampleOutboundInput();
  const referenceSdk = new EthereumMainnetSccp();
  const expectedRequest = referenceSdk.buildOutboundProofRequest(input);
  const wrongRequest = referenceSdk.buildOutboundProofRequest({
    ...sampleOutboundInput(),
    bundleBytes: [9, 8, 7],
  });
  const wrongProofResult = wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, wrongRequest);
  let seenRequest;
  const rejectingSdk = new EthereumMainnetSccp({
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
    outboundProver: {
      async prove(request) {
        acceptedRequest = request;
        return wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, request);
      },
    },
  });
  const proofResult = await acceptingSdk.proveOutboundToEthereum(input);
  assert.equal(acceptedRequest.requestHash, expectedRequest.requestHash);
  assert.equal(proofResult.requestHash, expectedRequest.requestHash);
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
  const sdk = new EthereumMainnetSccp({ executionProvider: provider });
  const request = sdk.buildOutboundProofRequest(sampleOutboundInput());
  const proofResult = wrapEvmSccpProofResult(GROTH16_PROOF_BYTES, request);

  assert.equal(await sdk.submitOutboundToEthereum({ proofResult }), "0xeth1");
  assert.equal(submittedTxs[0].to, request.destinationBinding.bridgeAddress);
  assert.equal(submittedTxs[0].data, sdk.buildEthereumCalldata({ proofResult }).callDataHex);

  const { destinationBinding, ...proofResultWithoutBinding } = proofResult;
  const { bridgeAddress: _bridgeAddress, ...bindingWithoutBridge } = destinationBinding;
  const snakeProofResult = {
    ...proofResultWithoutBinding,
    destination_binding: {
      ...bindingWithoutBridge,
      bridge_address: destinationBinding.bridgeAddress,
    },
  };
  assert.equal(await sdk.submitOutboundToEthereum({ proof_result: snakeProofResult }), "0xeth2");
  assert.equal(submittedTxs[1].to, request.destinationBinding.bridgeAddress);

  assert.equal(
    await sdk.submitOutboundToEthereum({
      proofResult,
      to: request.destinationBinding.bridgeAddress.toUpperCase(),
    }),
    "0xeth3",
  );
  assert.equal(submittedTxs[2].to, request.destinationBinding.bridgeAddress);

  assert.equal(
    await sdk.submitOutboundToEthereum({
      proofResult,
      from: `0x${"AA".repeat(20)}`,
    }),
    "0xeth4",
  );
  assert.equal(submittedTxs[3].from, `0x${"aa".repeat(20)}`);

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
