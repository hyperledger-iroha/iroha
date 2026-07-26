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
  "a79c4abe27cff185da6be1af15f0b7b36ca8f8729bd671eceed6758ec07a68ef79771ef904484f3ee3f8546f71aa8a40e99b5bf18820e80192491e32cedd9a01",
  "hex",
);
const signedTransactionHashHex = "6f39fd5e193f09f750939f0b089188b9a327a9dda0c8fb3de312c953bf2d93bb";

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
