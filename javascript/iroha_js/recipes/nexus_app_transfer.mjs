#!/usr/bin/env node
/**
 * Minimal Nexus App Facade transfer recipe.
 *
 * The fake Connect/Torii dependencies keep the recipe runnable without a live
 * wallet. In an app, configure NexusAppClient with its built-in browser Connect
 * path and Fetch-backed Torii client instead.
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
  "4bc83a65550abe7b583c68eada5de720a9487fd23ba00611669cb87c8e86bb437b87c871e39a178d286103e9f9328a17c5995ea7eaf048576aac62e0b6dcb503",
  "hex",
);
const signedTransactionHashHex = "d338123041fd61a734f21577b92cbe4b2c177541983ddc96e9e63f9fd878bde9";

const connectTransport = {
  async startConnect() {
    return {
      sid: "sid-demo-1",
      walletLaunchUri: "iroha://connect?sid=sid-demo-1&role=wallet",
    };
  },
  async awaitApproval(session) {
    return {
      accountId,
      signingPublicKey,
      session,
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
      hashHex: browserSignedTransactionHashHex(signedTransaction),
    };
  },
  async waitForTransactionStatus(hashHex) {
    return {
      hash: hashHex,
      status: { kind: "Applied", block_height: 1 },
      summary: "Applied",
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
