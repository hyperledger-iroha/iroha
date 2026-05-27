#!/usr/bin/env node
/**
 * Minimal Nexus App Facade transfer recipe.
 *
 * The fake Connect/Torii dependencies keep the recipe runnable without a live
 * wallet. Replace them with the browser Connect transport and ToriiClient in
 * an app.
 */
import { NexusAppClient } from "@iroha/iroha-js/nexus-app";

const accountId = "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB";
const destinationAccountId = "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L";
const sourceAssetId = `7EAD8EFYUx1aVKZPUU1fyKvr8dF1#${accountId}`;
const signingPublicKey = Buffer.from(
  "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737",
  "hex",
);
const payloadBytes = Buffer.from(
  "0c0b0a746573742d636861696e4f000000004a2100000000000000010001d0014a01b201320174012b01b401ab013a0113016801bd0146011501e401e601d00122014a01b7011a0101016b01af0185012001a3013201c9017701870137080068e5cf8b010000ac0200000000a60201000000000000009c020f0e69726f68612e7472616e736665728a0202010000000000004e5254300000a4174c78d6341f8f98fc2adae8ed67b900da000000000000006356adc8a15d041a0202000000d401764f000000004a2100000000000000010001d0014a01b201320174012b01b401ab013a0113016801bd0146011501e401e601d00122014a01b7011a0101016b01af0185012001a3013201c90177018701372001be01f5013c011c01cd0117014901e1018001df01ba01d60151019b01fd016604000000000c0602000000d20404020000004f000000004a2100000000000000010001a0019a01a501f4017a016701590180012f01f9015501f801dc012d012a011401a501c9019d012301be019701f801640112017f01f901380134015501a401f00a01083075000000000000060104070000002801000000000000001f0807707572706f7365151413226e657875732d6170702d6669787475726522",
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

const transactionCodec = {
  buildTransferPayload(input) {
    void input;
    return payloadBytes;
  },
  finalizeSignedTransaction(signable, signature) {
    const signedTransaction = Buffer.concat([
      Buffer.from("nexus-demo:"),
      signable.payloadBytes,
      signature.signature,
    ]);
    return {
      signedTransaction,
      hashHex: signedTransactionHashHex,
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
  sourceAssetId,
  quantity: "12.34",
  destinationAccountId,
  metadata: { purpose: "nexus-app-fixture" },
  creationTimeMs: 1_700_000_000_000,
  ttlMs: 30_000,
  nonce: 7,
});

console.log("wallet URI:", session.walletLaunchUri);
console.log("signed transaction hash:", receipt.signedTransactionHashHex);
console.log("final status:", receipt.status?.status ?? receipt.status);
