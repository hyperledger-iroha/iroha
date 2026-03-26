import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { buildBurnAssetTransaction } from "../src/transaction.js";
import { AccountAddress } from "../src/address.js";
import { blake2b256 } from "../src/blake2b.js";
import { makeNativeTest } from "./helpers/native.js";
import { getNativeBinding } from "../src/native.js";

const test = makeNativeTest(baseTest, {
  require: (binding) => typeof binding?.decodeSignedTransactionJson === "function",
});
const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..");

function loadJsonRelative(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  return JSON.parse(fs.readFileSync(absolutePath, "utf8"));
}

const canonicalManifest = loadJsonRelative(
  "fixtures/norito_rpc/transaction_fixtures.manifest.json",
);
function selectFixture(fixtures, name) {
  const match = fixtures.find((fixture) => fixture?.name === name);
  if (!match) {
    throw new Error(`Fixture '${name}' is missing from the manifest`);
  }
  return match;
}

const burnFixture = selectFixture(canonicalManifest.fixtures, "burn_asset");
const DEFAULT_SIGNING_SEED_HEX =
  "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
const signingSeedHex =
  canonicalManifest.signing_key?.seed_hex ?? DEFAULT_SIGNING_SEED_HEX;
const signingKeySeed = Buffer.from(signingSeedHex, "hex");
if (signingKeySeed.byteLength !== 32) {
  throw new Error("Fixture signing seed must be exactly 32 bytes (Ed25519)");
}

function extractCanonicalAuthority(payloadBase64, authorityHint) {
  if (typeof authorityHint === "string") {
    const trimmed = authorityHint.trim();
    if (trimmed.length > 0) {
      const atIndex = trimmed.lastIndexOf("@");
      if (atIndex !== -1) {
        const signatory = trimmed.slice(0, atIndex);
        if (signatory) {
          try {
            const { address } = AccountAddress.parseEncoded(signatory);
            return address.toI105();
          } catch {
            // Fall back to payload scan.
          }
        }
      } else {
        try {
          const { address } = AccountAddress.parseEncoded(trimmed);
          return address.toI105();
        } catch {
          // Fall back to payload scan.
        }
      }
    }
  }
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
  const prioritized = [
    ...printable.filter((entry) => entry.includes("@")),
    ...printable.filter((entry) => !entry.includes("@")),
  ];
  for (const entry of prioritized) {
    if (entry.includes("@")) {
      const [signatory] = entry.split("@", 2);
      if (!signatory) {
        continue;
      }
      try {
        const { address } = AccountAddress.parseEncoded(signatory);
        return address.toI105();
      } catch {
        continue;
      }
    }
    try {
      const { address } = AccountAddress.parseEncoded(entry);
      return address.toI105();
    } catch {
      continue;
    }
  }
  return null;
}

const canonicalAuthority =
  extractCanonicalAuthority(burnFixture.payload_base64, burnFixture.authority) ??
  (() => {
    throw new Error("Failed to extract canonical authority from fixture payload");
  })();

function irohaHashHex(bytes) {
  const digest = Buffer.from(blake2b256(bytes));
  if (digest.length > 0) {
    digest[digest.length - 1] |= 1;
  }
  return digest.toString("hex");
}

function decodeSignedPayload(bytes) {
  const binding = getNativeBinding();
  if (!binding || typeof binding.decodeSignedTransactionJson !== "function") {
    throw new Error("native binding decodeSignedTransactionJson is unavailable");
  }
  const json = binding.decodeSignedTransactionJson(bytes);
  if (!json) {
    throw new Error("native decoder returned null for signed transaction");
  }
  const decoded = JSON.parse(json);
  if (!decoded?.payload) {
    throw new Error("decoded signed transaction missing payload");
  }
  return decoded.payload;
}

baseTest("fixture base64 hashes match manifest", () => {
  for (const fixture of canonicalManifest.fixtures) {
    const payloadBytes = Buffer.from(fixture.payload_base64, "base64");
    assert.equal(
      payloadBytes.length,
      fixture.encoded_len,
      `${fixture.name}: payload length mismatch`,
    );
    assert.equal(
      irohaHashHex(payloadBytes),
      fixture.payload_hash,
      `${fixture.name}: payload hash mismatch`,
    );

    const signedBytes = Buffer.from(fixture.signed_base64, "base64");
    assert.equal(
      signedBytes.length,
      fixture.signed_len,
      `${fixture.name}: signed length mismatch`,
    );
    assert.equal(
      irohaHashHex(signedBytes),
      fixture.signed_hash,
      `${fixture.name}: signed hash mismatch`,
    );
  }
});

test("decoded fixture payloads match manifest metadata", () => {
  for (const fixture of canonicalManifest.fixtures) {
    const signedBytes = Buffer.from(fixture.signed_base64, "base64");
    const payload = decodeSignedPayload(signedBytes);
    assert.equal(
      payload.chain,
      fixture.chain,
      `${fixture.name}: chain mismatch in decoded payload`,
    );
    assert.equal(
      payload.authority,
      fixture.authority,
      `${fixture.name}: authority mismatch in decoded payload`,
    );
    assert.equal(
      payload.creation_time_ms,
      fixture.creation_time_ms,
      `${fixture.name}: creation_time_ms mismatch in decoded payload`,
    );
    assert.equal(
      payload.time_to_live_ms ?? null,
      fixture.time_to_live_ms ?? null,
      `${fixture.name}: time_to_live_ms mismatch in decoded payload`,
    );
    assert.equal(
      payload.nonce ?? null,
      fixture.nonce ?? null,
      `${fixture.name}: nonce mismatch in decoded payload`,
    );
  }
});

test("buildBurnAssetTransaction matches canonical Norito fixture", () => {
  const payloadBytes = Buffer.from(burnFixture.payload_base64, "base64");
  assert.equal(payloadBytes.length, burnFixture.encoded_len, "payload length mismatch");
  const signedBytes = Buffer.from(burnFixture.signed_base64, "base64");
  assert.equal(signedBytes.length, burnFixture.signed_len, "signed length mismatch");

  const fixturePayload = decodeSignedPayload(signedBytes);
  assert.equal(fixturePayload.chain, burnFixture.chain, "fixture chain mismatch");
  assert.equal(fixturePayload.authority, burnFixture.authority, "fixture authority mismatch");
  assert.equal(
    fixturePayload.creation_time_ms,
    burnFixture.creation_time_ms,
    "fixture creation_time_ms mismatch",
  );
  assert.equal(
    fixturePayload.time_to_live_ms ?? null,
    burnFixture.time_to_live_ms ?? null,
    "fixture time_to_live_ms mismatch",
  );
  assert.equal(
    fixturePayload.nonce ?? null,
    burnFixture.nonce ?? null,
    "fixture nonce mismatch",
  );

  const burnInstruction =
    fixturePayload.executable?.Instructions?.find((entry) => {
      const action = entry?.arguments?.action;
      return entry?.kind === "Burn" || action === "BurnAsset";
    }) ?? null;
  assert.ok(burnInstruction, "Burn fixture is missing instruction metadata");
  const assetId = burnInstruction.arguments?.asset;
  const canonicalAssetId =
    typeof assetId === "string"
      ? assetId.includes("##")
        ? assetId.replace(/##.+$/, `##${canonicalAuthority}`)
        : assetId.replace(/#[^#]+$/, `##${canonicalAuthority}`)
      : assetId;
  const quantity = burnInstruction.arguments?.quantity;
  assert.equal(typeof assetId, "string", "asset identifier must be provided");
  assert.ok(quantity !== undefined, "quantity must be provided in burn fixture");

  const { signedTransaction, hash } = buildBurnAssetTransaction({
    chainId: fixturePayload.chain,
    authority: canonicalAuthority,
    assetId: canonicalAssetId,
    quantity,
    metadata: fixturePayload.metadata ?? null,
    creationTimeMs: fixturePayload.creation_time_ms,
    ttlMs: fixturePayload.time_to_live_ms,
    nonce: fixturePayload.nonce ?? null,
    privateKey: signingKeySeed,
  });

  assert.equal(hash.length, 32, "hashSignedTransaction must return a 32-byte digest");
  assert.equal(
    signedTransaction.toString("base64"),
    burnFixture.signed_base64,
    "signed transaction mismatch",
  );
  assert.equal(hash.toString("hex"), burnFixture.signed_hash, "signed hash mismatch");

  const decoded = JSON.parse(getNativeBinding().decodeSignedTransactionJson(signedTransaction));
  assert.equal(decoded.payload.chain, fixturePayload.chain, "decoded chain id mismatch");
  assert.equal(decoded.payload.authority, canonicalAuthority, "decoded authority mismatch");
  assert.equal(
    decoded.payload.creation_time_ms,
    fixturePayload.creation_time_ms,
    "creation_time_ms mismatch",
  );
  assert.equal(
    decoded.payload.time_to_live_ms,
    fixturePayload.time_to_live_ms,
    "time_to_live_ms mismatch",
  );
  assert.equal(decoded.payload.nonce ?? null, fixturePayload.nonce ?? null, "nonce mismatch");
  assert.equal(decoded.payload.metadata?.memo, fixturePayload.metadata?.memo, "memo mismatch");
});
