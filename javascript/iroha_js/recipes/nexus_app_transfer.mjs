#!/usr/bin/env node
/**
 * Minimal Nexus App Facade transfer recipe.
 *
 * The fake Connect/Torii dependencies keep the recipe runnable without a live
 * wallet. Replace them with the browser Connect transport and ToriiClient in
 * an app.
 */
import { NexusAppClient } from "@iroha/iroha-js/nexus-app";

const signingPublicKey = Buffer.alloc(32, 0x11);
const walletSignature = Buffer.alloc(64, 0x07);

const connectTransport = {
  async startConnect() {
    return {
      sid: "sid-demo-1",
      walletLaunchUri: "iroha://connect?sid=sid-demo-1&role=wallet",
    };
  },
  async awaitApproval(session) {
    return {
      accountId: "sora-demo-account",
      signingPublicKey,
      session,
    };
  },
  async requestSignature(_session, signable) {
    console.log("payload hash:", signable.payloadHashHex);
    return { algorithm: "ed25519", signature: walletSignature };
  },
};

const transactionCodec = {
  buildTransferPayload(input) {
    return Buffer.from(JSON.stringify(input));
  },
  finalizeSignedTransaction(signable, signature) {
    const signedTransaction = Buffer.concat([
      Buffer.from("nexus-demo:"),
      signable.payloadBytes,
      signature.signature,
    ]);
    return {
      signedTransaction,
      hashHex: "demo-" + signable.payloadHashHex.slice(0, 16),
    };
  },
};

const toriiClient = {
  async submitTransaction() {
    return { accepted: true };
  },
  async waitForTransactionStatus(hashHex) {
    return { hash: hashHex, status: "Committed" };
  },
};

const client = new NexusAppClient({
  chainId: "test-chain",
  connectTransport,
  transactionCodec,
  toriiClient,
});

const session = await client.startConnect();
const approval = await client.awaitApproval(session);
const receipt = await client.transferWithWallet(approval.session, {
  sourceAssetId: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1#sora-demo-account",
  quantity: "12.34",
  destinationAccountId: "sora-destination-account",
  creationTimeMs: 1_700_000_000_000,
  ttlMs: 30_000,
  nonce: 7,
});

console.log("wallet URI:", session.walletLaunchUri);
console.log("signed transaction hash:", receipt.signedTransactionHashHex);
console.log("final status:", receipt.status?.status ?? receipt.status);
