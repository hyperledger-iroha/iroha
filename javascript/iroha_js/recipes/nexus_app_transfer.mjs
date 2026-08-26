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
  AccountAddress,
  NetworkId,
  generateKeyPair,
  signEd25519,
} from "@iroha/iroha-js";
import {
  browserSignedTransactionHashHex,
  browserTransactionCodec,
} from "@iroha/iroha-js/transaction-codec";

const walletKeyPair = generateKeyPair({ seed: Buffer.alloc(32, 0x11) });
const accountId = AccountAddress.fromAccount({
  publicKey: walletKeyPair.publicKey,
}).toI105(369);
const destinationAccountId = AccountAddress.parseEncoded(
  "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L",
).address.toI105(369);
const sourceAssetId = `7EAD8EFYUx1aVKZPUU1fyKvr8dF1#${accountId}`;
const networkId = NetworkId.parse(
  "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149",
);
const signingPublicKey = walletKeyPair.publicKey;
const signingPrivateKey = walletKeyPair.privateKey;
const signedTransactionHashHex = "4698dbf7f4a250029037275258fbaca1095bb476747929b279b02c1b0236d1d7";

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
    return {
      algorithm: "ed25519",
      signature: signEd25519(
        Buffer.from(signable.payloadHashHex, "hex"),
        signingPrivateKey,
      ),
    };
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
  throw new Error(
    `recipe signed transaction hash ${receipt.signedTransactionHashHex} drifted from the shared fixture`,
  );
}
console.log("wallet URI:", session.walletLaunchUri);
console.log("signed transaction hash:", receipt.signedTransactionHashHex);
const finalStatus = receipt.status?.status ?? receipt.status;
console.log("final status:", finalStatus?.kind ?? finalStatus);
