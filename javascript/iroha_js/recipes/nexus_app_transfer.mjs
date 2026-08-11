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

const accountId = "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB";
const destinationAccountId = "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L";
const sourceAssetId = `7EAD8EFYUx1aVKZPUU1fyKvr8dF1#${accountId}`;
const networkId = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const signingPublicKey = Buffer.from(
  "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737",
  "hex",
);
const walletSignature = Buffer.from(
  "d39065822f28108f70f8089f64357cc33a0072e45aa65f6b3e2696b93a3d9779d376ddf19c8e7dabce79a484275b681dea5213df060848d8fe098edeebcc3c07",
  "hex",
);
const signedTransactionHashHex = "b410d55b960d396c1034221dea22464d08de1237363b02cb1f7c35d4c6eaf0a1";

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
