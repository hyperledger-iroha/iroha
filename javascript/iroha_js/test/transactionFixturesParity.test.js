import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { buildBurnAssetTransaction } from "../src/transaction.js";
import { AccountAddress } from "../src/address.js";
import { makeNativeTest } from "./helpers/native.js";
import { getNativeBinding } from "../src/native.js";

const test = makeNativeTest(baseTest);
const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..");

function loadJsonRelative(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  return JSON.parse(fs.readFileSync(absolutePath, "utf8"));
}

const canonicalManifest = loadJsonRelative(
  "fixtures/norito_rpc/transaction_fixtures.manifest.json",
);
const swiftPayloads = loadJsonRelative("IrohaSwift/Fixtures/transaction_payloads.json");

function selectFixture(fixtures, name) {
  const match = fixtures.find((fixture) => fixture?.name === name);
  if (!match) {
    throw new Error(`Fixture '${name}' is missing from the manifest`);
  }
  return match;
}

const burnFixture = selectFixture(canonicalManifest.fixtures, "burn_asset");
const burnPayload = selectFixture(swiftPayloads, "burn_asset").payload;

const signingSeedHex = canonicalManifest.signing_key?.seed_hex;
if (!signingSeedHex) {
  throw new Error("Canonical manifest is missing signing_key.seed_hex");
}
const signingKeySeed = Buffer.from(signingSeedHex, "hex");
if (signingKeySeed.byteLength !== 32) {
  throw new Error("Fixture signing seed must be exactly 32 bytes (Ed25519)");
}

function extractCanonicalAuthority(payloadBase64) {
  const bytes = Buffer.from(payloadBase64, "base64");
  const printable = [];
  let current = "";
  for (const byte of bytes) {
    const ch = byte >= 0x20 && byte <= 0x7e ? String.fromCharCode(byte) : null;
    if (ch && /[A-Za-z0-9@._#]/.test(ch)) {
      current += ch;
      continue;
    }
    if (current.length >= 8) {
      printable.push(current);
    }
    current = "";
  }
  if (current.length >= 8) {
    printable.push(current);
  }
  const authority = printable.find((entry) => entry.includes("@")) ?? null;
  if (!authority) {
    return null;
  }
  const [signatory, domain] = authority.split("@", 2);
  if (!signatory || !domain) {
    return null;
  }
  const { address } = AccountAddress.parseAny(signatory, undefined, domain);
  return `${address.toIH58()}@${domain}`;
}

const canonicalAuthority =
  extractCanonicalAuthority(burnFixture.payload_base64) ??
  (() => {
    throw new Error("Failed to extract canonical authority from fixture payload");
  })();

test("buildBurnAssetTransaction matches canonical Norito fixture", () => {
  const payloadBytes = Buffer.from(burnFixture.payload_base64, "base64");
  assert.equal(payloadBytes.length, burnFixture.encoded_len, "payload length mismatch");
  const signedBytes = Buffer.from(burnFixture.signed_base64, "base64");
  assert.equal(signedBytes.length, burnFixture.signed_len, "signed length mismatch");

  const burnInstruction =
    burnPayload.executable?.Instructions?.find((entry) => {
      const action = entry?.arguments?.action;
      return entry?.kind === "Burn" || action === "BurnAsset";
    }) ?? null;
  assert.ok(burnInstruction, "Burn fixture is missing instruction metadata");
  const assetId = burnInstruction.arguments?.asset;
  const canonicalAssetId =
    typeof assetId === "string"
      ? assetId.includes("##")
        ? assetId.replace(/##.+$/, `##${canonicalAuthority}`)
        : assetId.replace(/#[^#]+$/, `#${canonicalAuthority}`)
      : assetId;
  const quantity = burnInstruction.arguments?.quantity;
  assert.equal(typeof assetId, "string", "asset identifier must be provided");
  assert.ok(quantity !== undefined, "quantity must be provided in burn fixture");

  const { signedTransaction, hash } = buildBurnAssetTransaction({
    chainId: burnPayload.chain,
    authority: canonicalAuthority,
    assetId: canonicalAssetId,
    quantity,
    metadata: burnPayload.metadata ?? null,
    creationTimeMs: burnPayload.creation_time_ms,
    ttlMs: burnPayload.time_to_live_ms,
    nonce: burnPayload.nonce ?? null,
    privateKey: signingKeySeed,
  });

  assert.equal(hash.length, 32, "hashSignedTransaction must return a 32-byte digest");

  const decoded = JSON.parse(getNativeBinding().decodeSignedTransactionJson(signedTransaction));
  assert.equal(decoded.payload.chain, burnPayload.chain, "decoded chain id mismatch");
  assert.equal(decoded.payload.metadata?.memo, burnPayload.metadata?.memo, "memo mismatch");
});
