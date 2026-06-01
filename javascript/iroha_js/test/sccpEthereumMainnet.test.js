import { test } from "node:test";
import assert from "node:assert/strict";
import {
  EthereumMainnetSccp,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_ETH,
  SCCP_DOMAIN_SORA,
  SCCP_ETH_MAINNET_EVM_CHAIN_ID,
  SCCP_ETH_MAINNET_NETWORK_ID,
  SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1,
  ethereumMainnetSccpDestinationBinding,
  wrapEvmSccpProofResult,
} from "../src/sccp.js";

const hex32 = (byte) => `0x${byte.repeat(32)}`;
const TX_HASH = hex32("aa");
const BLOCK_HASH = hex32("bb");

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

const sampleOutboundInput = (targetDomain = SCCP_DOMAIN_ETH) => ({
  publicInputs: { ...samplePublicInputs, targetDomain },
  bundleBytes: [1, 2, 3],
  destinationBinding: ethereumMainnetSccpDestinationBinding(sampleDestinationBindingInput()),
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

test("EthereumMainnetSccp rejects padded JSON-RPC chain ids", async () => {
  const sdk = new EthereumMainnetSccp({
    executionProvider: {
      async request() {
        return "0x01";
      },
    },
  });

  await assert.rejects(
    () => sdk.validateExecutionProviderMainnet(),
    /hex, decimal, number, or bigint chain id/u,
  );
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
  assert.equal(evidence.beaconFinality.executionBlockNumber, "4660");
  assert.equal(evidence.beaconFinality.executionBlockHash, BLOCK_HASH);
  assert.equal(evidence.beaconFinality.executionReceiptsRoot, hex32("cc"));
  assert.deepEqual(
    calls.map(([method]) => method),
    ["eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"],
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
          };
        }
        if (method === "eth_getBlockByHash") {
          return { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") };
        }
        throw new Error(`unexpected RPC method ${method}`);
      },
    },
    consensusProvider: {
      collectFinalityEvidence() {
        return {
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
      return [1, 2, 3];
    },
  });

  assert.deepEqual(
    await sdk.proveInboundToSora({ transactionHash: TX_HASH }),
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
      }).proveInboundToSora({ sourceDomain: SCCP_DOMAIN_ETH, targetDomain: SCCP_DOMAIN_SORA }),
    /requires a receipt, receiptProof, receiptProofHash, or transactionHash/u,
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
        },
        block: { hash: BLOCK_HASH, number: "0x1234", receiptsRoot: hex32("cc") },
      }),
    /requires beaconFinality/u,
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

test("EthereumMainnetSccp keeps the easy outbound path Ethereum-only", () => {
  const sdk = new EthereumMainnetSccp();
  const ethRequest = sdk.buildOutboundProofRequest(sampleOutboundInput());
  assert.equal(ethRequest.targetDomain, SCCP_DOMAIN_ETH);
  assert.equal(ethRequest.destinationBinding.networkId, SCCP_ETH_MAINNET_NETWORK_ID);

  assert.throws(
    () => sdk.buildOutboundProofRequest(sampleOutboundInput(SCCP_DOMAIN_BSC)),
    /request route|targetDomain|Ethereum mainnet/u,
  );
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
