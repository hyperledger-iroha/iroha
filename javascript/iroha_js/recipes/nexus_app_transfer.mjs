#!/usr/bin/env node
/**
 * Minimal Nexus App Facade transfer recipe.
 *
 * The fake Connect/Torii dependencies keep the recipe runnable without a live
 * wallet. In an app, configure NexusAppClient with its built-in browser Connect
 * path and Fetch-backed Torii client instead.
 */
import { NexusAppClient } from "@iroha/iroha-js/nexus-app";
import {
  browserSignedTransactionHashHex,
  browserTransactionCodec,
} from "@iroha/iroha-js/transaction-codec";

const accountId = "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB";
const destinationAccountId = "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L";
const sourceAssetId = `7EAD8EFYUx1aVKZPUU1fyKvr8dF1#${accountId}`;
const signingPublicKey = Buffer.from(
  "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737",
  "hex",
);
const walletSignature = Buffer.from(
  "c82d2ee732a9251153eff6f510a0d12b292cb51a5d961a7eddb84f6ee944e34eaca60ca2f1ccfe7a53fd6813fc9a6db9e35cb276b2411b7d583d45fdc6caee05",
  "hex",
);
const signedTransactionHashHex = "2d22bf944c58886de938e4094bf9887a43e66d598162bd2205f0812b64e180bb";

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
    return { hash: hashHex, status: "Committed" };
  },
};

const client = new NexusAppClient({
  chainId: "test-chain",
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
console.log("final status:", receipt.status?.status ?? receipt.status);
