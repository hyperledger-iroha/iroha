#!/usr/bin/env node
/**
 * Minimal Nexus App Facade transfer recipe.
 *
 * Run in Node with the SDK's verified native binding installed. The fake
 * Connect/Torii dependencies avoid live wallet or network requests; canonical
 * account validation and transaction encoding still use the native codec.
 */
import { NexusAppClient } from "@iroha/iroha-js/nexus-app";
import { NetworkId } from "@iroha/iroha-js";
import {
  browserSignedTransactionHashHex,
  browserTransactionCodec,
} from "@iroha/iroha-js/transaction-codec";

const accountChainDiscriminant = 369;
const accountId = "testuﾛ1PﾀR2LBﾃﾋQ8ﾅﾚHｱﾍmtX5Aﾉｽ2ｽヱﾙVｳﾁoJXWpﾄﾖFｸｼ8RC99U";
const destinationAccountId = "testuﾛ1Nﾛ5ﾃPefCWUﾆﾔaxCRﾈﾅｶubGPﾘｼX9hﾀ8vHGVﾗsﾒJｼF7HF5W";
const sourceAssetId = `53SSUt68Qn5PdKMMrViDK57X6DG2#${accountId}`;
const networkId = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const signingPublicKey = Buffer.from(
  "c050c5637a44fa8629fff3cccce2300cb362a63d99d95fc54145266f4332445a",
  "hex",
);
const walletSignature = Buffer.from(
  "f0facd1407187402d6d3de380e44bdc157fa81601ea057df525f2284fbb29a65e7b0f9c237339dd64057503f679e733ea44f74fc277f02eaec60b7604b103607",
  "hex",
);
const signedTransactionHashHex = "ef1e1042fb07356fa96d6d51898837f5d2030b8e6286acb1c31076501360aad5";

const connectTransport = {
  async startConnect() {
    return {
      sid: "sid-demo-1",
      walletLaunchUri: "iroha://connect?sid=sid-demo-1&role=wallet",
    };
  },
  async awaitApproval(_session) {
    return {
      accountId,
      signingPublicKey,
    };
  },
  async requestSignature(_session, signable) {
    console.log("payload hash:", signable.payloadHashHex);
    return { algorithm: "ed25519", signature: walletSignature };
  },
};

const toriiClient = {
  async submitTransaction(signedTransaction) {
    return {
      accepted: true,
      hashHex: browserSignedTransactionHashHex(signedTransaction, accountChainDiscriminant),
    };
  },
  async waitForTransactionStatus(hashHex) {
    return {
      hash: hashHex,
      status: { kind: "Applied", block_height: 1 },
      diagnostics: [],
      scope: "global",
      resolved_from: "state",
    };
  },
};

const client = new NexusAppClient({
  networkId,
  chainDiscriminant: accountChainDiscriminant,
  connectTransport,
  transactionCodec: browserTransactionCodec,
  toriiClient,
});

const session = await client.startConnect();
const approval = await client.awaitApproval(session);
const receipt = await client.transferWithWallet(approval.session, {
  sourceAssetId,
  quantity: "12.34",
  destinationAccountId,
  feePayment: { payer: "authority", chargeLimits: [] },
  metadata: { purpose: "nexus-app-fixture" },
  creationTimeMs: 1_700_000_000_000,
  ttlMs: 30_000,
  nonce: 7,
});

if (receipt.signedTransactionHashHex !== signedTransactionHashHex) {
  throw new Error("recipe signed transaction hash drifted from the shared fixture");
}
console.log("wallet URI:", session.walletLaunchUri);
console.log("signed transaction hash:", receipt.signedTransactionHashHex);
const finalStatus = receipt.status?.status ?? receipt.status;
console.log("final status:", finalStatus?.kind ?? finalStatus);
