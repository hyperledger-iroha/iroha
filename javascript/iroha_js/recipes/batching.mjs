#!/usr/bin/env node

import { Buffer } from "node:buffer";
import { ed25519 } from "@noble/curves/ed25519";
import {
  AccountAddress,
  NetworkId,
  buildMintAssetInstruction,
  buildBurnAssetInstruction,
  buildTransferAssetInstruction,
  buildMintAndTransferTransaction,
  buildRegisterAssetDefinitionMintAndTransferTransaction,
  buildTransaction,
  noritoEncodeInstruction,
  noritoDecodeInstruction,
} from "../src/index.js";

const networkId = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const privateKey = Buffer.from(
  "CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
  "hex",
);
const authority = AccountAddress.fromAccount({
  publicKey: Buffer.from(ed25519.getPublicKey(privateKey)),
}).toI105();
const recipient =
  "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L";
const assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const assetId = `${assetDefinitionId}#${authority}`;
// Local encoding-only intent. Live submissions must replace the empty maxima
// through `quoteAndSignTransaction` before signing.
const feePayment = { payer: "authority", chargeLimits: [] };

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
  networkId,
  authority,
  feePayment,
  instructions: [mintInstruction, transferInstruction, burnInstruction],
  nonce: 1,
  ttlMs: 60_000,
  privateKey,
});

console.log(`\\nManual batch hash: ${manualBatch.hash.toString("hex")}`);

const mintAndTransfer = buildMintAndTransferTransaction({
  networkId,
  authority,
  feePayment,
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
    networkId,
    authority,
    feePayment,
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
