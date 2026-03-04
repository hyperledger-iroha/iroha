#!/usr/bin/env node

import { Buffer } from "node:buffer";
import {
  buildMintAssetInstruction,
  buildBurnAssetInstruction,
  buildTransferAssetInstruction,
  buildMintAndTransferTransaction,
  buildRegisterAssetDefinitionMintAndTransferTransaction,
  buildTransaction,
  noritoEncodeInstruction,
  noritoDecodeInstruction,
} from "../src/index.js";

const chainId = "batching-sample-chain";
const authority =
  "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245@wonderland";
const recipient =
  "ed0120C0F6FA775885F8FFB5F203C10EAA90E1B49FB4CD39C0F95CCA1E5A02B5E45F61@wonderland";
const assetDefinitionId = "rose#wonderland";
const assetId = `${assetDefinitionId}#${authority}`;
const privateKey = Buffer.alloc(32, 0x11);

function inspectInstruction(label, instruction) {
  const decoded = noritoDecodeInstruction(noritoEncodeInstruction(instruction));
  console.log(`\\n=== ${label} ===`);
  console.log(JSON.stringify(decoded, null, 2));
}

const mintInstruction = buildMintAssetInstruction({
  assetId,
  quantity: "10",
});
const transferInstruction = buildTransferAssetInstruction({
  sourceAssetId: assetId,
  quantity: "4",
  destinationAccountId: recipient,
});
const burnInstruction = buildBurnAssetInstruction({
  assetId,
  quantity: "2",
});

inspectInstruction("Mint", mintInstruction);
inspectInstruction("Transfer", transferInstruction);
inspectInstruction("Burn", burnInstruction);

const manualBatch = buildTransaction({
  chainId,
  authority,
  instructions: [mintInstruction, transferInstruction, burnInstruction],
  nonce: 1,
  ttlMs: 60_000,
  privateKey,
});

console.log(`\\nManual batch hash: ${manualBatch.hash.toString("hex")}`);

const mintAndTransfer = buildMintAndTransferTransaction({
  chainId,
  authority,
  mint: { assetId, quantity: "10" },
  transfers: [
    { quantity: "5", destinationAccountId: authority },
    { sourceAssetId: assetId, quantity: "3", destinationAccountId: recipient },
  ],
  nonce: 2,
  ttlMs: 60_000,
  privateKey,
});

console.log(
  `Mint + transfer helper hash: ${mintAndTransfer.hash.toString("hex")}`,
);

const registerMintTransfer = buildRegisterAssetDefinitionMintAndTransferTransaction(
  {
    chainId,
    authority,
    assetDefinition: {
      assetDefinitionId,
      metadata: { description: "Sample asset" },
    },
    mint: { accountId: authority, quantity: "6" },
    transfers: [{ quantity: "2", destinationAccountId: recipient }],
    nonce: 3,
    ttlMs: 60_000,
    privateKey,
  },
);

console.log(
  `Register + mint + transfer hash: ${registerMintTransfer.hash.toString("hex")}`,
);
