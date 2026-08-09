import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import {
  buildKaigiRosterJoinProof,
  generateKeyPair,
  normalizeCryptoAlgorithm,
  supportedCryptoAlgorithms,
} from "../src/crypto.browser.js";
import * as srcBrowserCrypto from "../src/crypto.browser.js";
import * as distBrowserCrypto from "../dist/crypto.browser.js";
import * as srcBrowserFacade from "../src/browser.js";
import * as distBrowserFacade from "../dist/browser.js";
import * as srcPrivacyCapabilities from "../src/privacyCapabilities.js";
import * as distPrivacyCapabilities from "../dist/privacyCapabilities.js";

test("browser crypto bundle exposes Kaigi roster proof helper and omits retired ZK-ACE helpers", () => {
  assert.throws(
    () => buildKaigiRosterJoinProof({ seed: Buffer.from("seed") }),
    /buildKaigiRosterJoinProof is unavailable in browser-only crypto builds/,
  );
  for (const [label, crypto] of [
    ["src", srcBrowserCrypto],
    ["dist", distBrowserCrypto],
  ]) {
    assert.equal(
      "buildZkAceTransferAuthorizationV1" in crypto,
      false,
      `${label} retired ZK-ACE builder must be absent`,
    );
  }
});

test("browser crypto normalizes all algorithm labels but only signs Ed25519 locally", () => {
  assert.ok(supportedCryptoAlgorithms().includes("ml-dsa"));
  assert.equal(normalizeCryptoAlgorithm("gost3410-2012-256-paramset-a"), "gost3410-2012-256-paramset-a");
  for (const [label, crypto] of [
    ["src", srcBrowserCrypto],
    ["dist", distBrowserCrypto],
  ]) {
    assert.equal(crypto.normalizeCryptoAlgorithm("ed-25519"), "ed25519", `${label} keeps ASCII aliases`);
    assert.throws(
      () => crypto.normalizeCryptoAlgorithm("ed\t25519"),
      /unsupported crypto algorithm/,
      `${label} rejects control-character aliases`,
    );
    assert.throws(
      () => crypto.normalizeCryptoAlgorithm("ed\u200B25519"),
      /unsupported crypto algorithm/,
      `${label} rejects zero-width aliases`,
    );
    assert.throws(
      () => crypto.normalizeCryptoAlgorithm("\u0435d25519"),
      /unsupported crypto algorithm/,
      `${label} rejects Cyrillic aliases`,
    );
  }
  assert.equal(generateKeyPair({ seed: Buffer.alloc(32, 7) }).algorithm, "ed25519");
  assert.throws(
    () => generateKeyPair({ seed: Buffer.alloc(16, 7) }),
    /seed must be exactly 32 bytes/,
  );
  assert.throws(
    () => generateKeyPair({ algorithm: "ml-dsa", seed: Buffer.alloc(32, 7) }),
    /generateKeyPair\(ml-dsa\) is unavailable in browser-only crypto builds/,
  );
});

test("browser crypto exposes recovery phrase helpers in source and dist bundles", () => {
  const entropy = Buffer.from(Array.from({ length: 32 }, (_, index) => index + 1));

  for (const [label, crypto] of [
    ["src", srcBrowserCrypto],
    ["dist", distBrowserCrypto],
  ]) {
    const recovery = crypto.ed25519SeedToRecoveryPhrase(entropy);

    assert.equal(recovery.wordCount, 24, `${label} exports 24-word Ed25519 seed recovery`);
    assert.equal(recovery.words.length, 24, `${label} recovery word count matches`);
    assert.equal(crypto.validateRecoveryPhrase(recovery.phrase), true, `${label} validates generated phrase`);
    assert.deepEqual(crypto.recoveryPhraseToEntropy(recovery.phrase), entropy, `${label} recovers original seed`);
    assert.deepEqual(
      crypto.deriveEd25519SeedFromRecoveryPhrase(recovery.phrase),
      entropy,
      `${label} derives original Ed25519 seed`,
    );
    const shortEntropy = Buffer.alloc(16, 7);
    const shortRecovery = crypto.entropyToRecoveryPhrase(shortEntropy);
    assert.equal(
      crypto
        .deriveEd25519SeedFromRecoveryPhrase(shortRecovery.phrase)
        .toString("hex"),
      "d761d406af2a4a5a15f67c924378ed88d1f85c13f1a37fc7366f59789b3bcd65",
      `${label} expands 12-word entropy deterministically`,
    );
    assert.throws(
      () => crypto.normalizeRecoveryPhrase(recovery.words.slice(0, 11).join(" ")),
      /12 or 24 words/,
      `${label} rejects unsupported word count`,
    );
    assert.throws(
      () => crypto.entropyToRecoveryPhrase(Buffer.alloc(20)),
      /16 or 32 bytes/,
      `${label} rejects unsupported entropy length`,
    );
  }
});

test("mapped browser crypto keeps the native-only local catalog fail closed", () => {
  for (const [label, crypto] of [["src", srcBrowserCrypto], ["dist", distBrowserCrypto]]) {
    assert.equal(crypto.isPrivacyNativeAvailable(), false, `${label} privacy bridge must be unavailable`);
    assert.throws(
      () => crypto.privacyCompiledProfileCatalogV1(),
      /unavailable in browser-only crypto builds/,
    );
    for (const retired of [
      "privacyCapabilitiesV1",
      "privacyProofRequestV1",
      "privacyBuildProofV1",
      "privacyVerifyProofV1",
    ]) {
      assert.equal(retired in crypto, false, `${label} ${retired} must be retired`);
    }
  }
});

test("broad browser facade omits the native catalog and retains the live Torii parser subpath", () => {
  for (const [label, browser] of [
    ["src", srcBrowserFacade],
    ["dist", distBrowserFacade],
  ]) {
    assert.equal(
      "privacyCompiledProfileCatalogV1" in browser,
      false,
      `${label} broad browser facade must omit native build metadata`,
    );
    assert.equal("privacyCapabilitiesV1" in browser, false);
  }
  for (const [label, capabilities] of [
    ["src", srcPrivacyCapabilities],
    ["dist", distPrivacyCapabilities],
  ]) {
    assert.equal(
      typeof capabilities.getPrivacyCapabilitiesV1,
      "function",
      `${label} keeps the live Torii capability client`,
    );
    assert.equal(
      typeof capabilities.parsePrivacyCapabilitySnapshotV1,
      "function",
      `${label} keeps the authoritative snapshot parser`,
    );
  }
});

test("browser crypto covers the package root crypto export surface", () => {
  const indexSource = readFileSync(new URL("../src/index.js", import.meta.url), "utf8");
  const browserCryptoSource = readFileSync(
    new URL("../src/crypto.browser.js", import.meta.url),
    "utf8",
  );
  const exportBlocks = [
    ...indexSource.matchAll(/export\s+\{([\s\S]*?)\}\s+from\s+"([^"]+)";/g),
  ];
  const cryptoExportBlock = exportBlocks.find((match) => match[2] === "./crypto.js");
  assert.ok(cryptoExportBlock);
  const rootCryptoExports = cryptoExportBlock[1]
    .split(",")
    .map((name) => name.trim())
    .filter(Boolean);
  const browserExports = new Set(
    [...browserCryptoSource.matchAll(/export\s+(?:const|function|class)\s+([A-Za-z0-9_]+)/g)]
      .map((match) => match[1]),
  );
  assert.deepEqual(
    rootCryptoExports.filter((name) => !browserExports.has(name)),
    [],
  );
});

test("browser package wiring omits the retired privacy catalog module", () => {
  const packageJson = JSON.parse(
    readFileSync(new URL("../package.json", import.meta.url), "utf8"),
  );
  assert.equal(packageJson.exports["./crypto"].browser, "./dist/crypto.browser.js");
  assert.equal(packageJson.browser["./dist/crypto.js"], "./dist/crypto.browser.js");

  for (const [label, relativePath] of [
    ["src", "../src/privacyAlgorithms.js"],
    ["dist", "../dist/privacyAlgorithms.js"],
  ]) {
    assert.equal(
      existsSync(new URL(relativePath, import.meta.url)),
      false,
      `${label} privacy catalog must remain deleted`,
    );
  }
});
