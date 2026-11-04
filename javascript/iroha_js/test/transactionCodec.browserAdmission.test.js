import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { ed25519 } from "@noble/curves/ed25519.js";
import { AccountAddress } from "../src/address.js";
import { NetworkId } from "../src/networkId.js";
import {
  browserSignedTransactionHashHex,
  browserTransactionPayloadHashHex,
  buildBrowserTransferPayload,
  finalizeBrowserSignedTransaction,
} from "../src/transactionCodec.js";

const SRC = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "src");
const PRIVATE_KEY = Buffer.from(
  "CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
  "hex",
);
const PUBLIC_KEY = Buffer.from(ed25519.getPublicKey(PRIVATE_KEY));
const NETWORK_ID = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);

function i105(publicKey) {
  return AccountAddress.fromAccount({ algorithm: "ed25519", publicKey }).toI105();
}

function signedTransfer() {
  const authority = i105(PUBLIC_KEY);
  const payload = buildBrowserTransferPayload({
    networkPrefix: 753,
    networkId: NETWORK_ID,
    authority,
    sourceAssetHoldingId: `62Fk4FPcMuLvW5QjDGNF2a4jAmjM#${authority}`,
    quantity: "1.25",
    destinationAccountId: i105(
      Buffer.from("641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201", "hex"),
    ),
    feePayment: { payer: "authority", chargeLimits: [] },
    metadata: {},
    creationTimeMs: 1_700_000_000_000,
    ttlMs: 5_000,
    nonce: 42,
  });
  const payloadHashHex = browserTransactionPayloadHashHex(payload, 753);
  const signature = Buffer.from(ed25519.sign(Buffer.from(payloadHashHex, "hex"), PRIVATE_KEY));
  return finalizeBrowserSignedTransaction(
    {
      networkPrefix: 753,
      networkId: NETWORK_ID,
      payloadBytes: payload,
      payloadHashHex,
      authority,
      signingPublicKey: PUBLIC_KEY,
      signatureAlgorithm: "ed25519",
    },
    { algorithm: "ed25519", signature },
    PUBLIC_KEY,
  );
}

// Resolve the SDK's own `native.js` to `native.browser.js`, exactly as a browser bundle
// does through the package `browser` field, so any native codec call throws.
const BROWSER_NATIVE_HOOK = `
import { registerHooks } from "node:module";
const nativeUrl = ${JSON.stringify(pathToFileURL(path.join(SRC, "native.js")).href)};
const browserUrl = ${JSON.stringify(pathToFileURL(path.join(SRC, "native.browser.js")).href)};
registerHooks({
  resolve(specifier, context, nextResolve) {
    const resolved = nextResolve(specifier, context);
    return resolved.url === nativeUrl ? { ...resolved, url: browserUrl } : resolved;
  },
});
`;

function hashInBrowserMode(signedTransaction) {
  const codecUrl = pathToFileURL(path.join(SRC, "transactionCodec.js")).href;
  const script = `
    const { browserSignedTransactionHashHex } = await import(${JSON.stringify(codecUrl)});
    const bytes = Uint8Array.from(Buffer.from(${JSON.stringify(Buffer.from(signedTransaction).toString("base64"))}, "base64"));
    try {
      process.stdout.write(JSON.stringify({ hashHex: browserSignedTransactionHashHex(bytes, 753) }));
    } catch (error) {
      process.stdout.write(JSON.stringify({ error: String(error?.message ?? error) }));
    }
  `;
  const child = spawnSync(
    process.execPath,
    [
      "--import",
      `data:text/javascript,${encodeURIComponent(BROWSER_NATIVE_HOOK)}`,
      "--input-type=module",
      "--eval",
      script,
    ],
    { encoding: "utf8" },
  );
  assert.equal(child.status, 0, child.stderr);
  return JSON.parse(child.stdout);
}

test("browser signed-transaction admission does not require the native codec", () => {
  const signed = signedTransfer();
  assert.equal(browserSignedTransactionHashHex(signed.signedTransaction, 753), signed.hashHex);
  assert.deepEqual(hashInBrowserMode(signed.signedTransaction), { hashHex: signed.hashHex });
});
