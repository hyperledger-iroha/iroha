import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  buildKaigiRosterJoinProof,
  buildZkAceTransferAuthorizationV1,
  generateKeyPair,
  isKagemushaRecursiveSpendNativeAvailable,
  kagemushaRecursiveSpendInit,
  KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
  KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1,
  normalizeCryptoAlgorithm,
  preferredKagemushaOfflineSpendMode,
  preferredKagemushaOfflineSpendModeForCapabilities,
  supportedCryptoAlgorithms,
} from "../src/crypto.browser.js";
import * as srcBrowserCrypto from "../src/crypto.browser.js";
import * as distBrowserCrypto from "../dist/crypto.browser.js";

test("browser crypto bundle exposes Kaigi roster proof helper as unsupported", () => {
  assert.throws(
    () => buildKaigiRosterJoinProof({ seed: Buffer.from("seed") }),
    /buildKaigiRosterJoinProof is unavailable in browser-only crypto builds/,
  );
  assert.throws(
    () => buildZkAceTransferAuthorizationV1({}),
    /buildZkAceTransferAuthorizationV1 is unavailable in browser-only crypto builds/,
  );
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
    () => generateKeyPair({ algorithm: "ml-dsa", seed: Buffer.alloc(32, 7) }),
    /generateKeyPair\(ml-dsa\) is unavailable in browser-only crypto builds/,
  );
});

test("browser crypto exposes native-only helpers as safe stubs", () => {
  for (const [label, crypto] of [
    ["src", srcBrowserCrypto],
    ["dist", distBrowserCrypto],
  ]) {
    assert.equal(crypto.isPrivacyNativeAvailable(), false, `${label} privacy bridge must be unavailable`);
    assert.equal(
      crypto.isKagemushaRecursiveSpendNativeAvailable(),
      false,
      `${label} Kagemusha native bridge must be unavailable`,
    );
    assert.equal(
      crypto.preferredKagemushaOfflineSpendMode(),
      crypto.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
      `${label} browser build must default to checked prefold spend mode`,
    );
    assert.equal(
      crypto.preferredKagemushaOfflineSpendMode(true),
      crypto.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
      `${label} native recursive availability should select recursive spend mode`,
    );
    assert.equal(
      crypto.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1,
      "recursive_compact_v1",
      `${label} exposes recursive compact spend mode`,
    );
    assert.equal(
      crypto.preferredKagemushaOfflineSpendModeForCapabilities(true, true),
      crypto.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
      `${label} capability helper should prefer recursive spend mode`,
    );
    assert.equal(
      crypto.isKagemushaRecursiveCompactPaymentTokenNativeAvailable(),
      false,
      `${label} browser build must not expose native recursive compact prover`,
    );
    assert.equal(
      crypto.isKagemushaCompactPaymentTokenNativeAvailable(),
      false,
      `${label} browser build must not expose native compact-token prover`,
    );
    assert.equal(
      crypto.isKagemushaRecursiveAggregationProofBundleNativeAvailable(),
      false,
      `${label} browser build must not expose native recursive aggregation prover`,
    );
    assert.throws(
      () => crypto.kagemushaProveVerifiedCompactPaymentTokenWithRecords(),
      /unavailable in browser-only crypto builds/,
      `${label} compact-token prover must be native-only`,
    );
    assert.throws(
      () =>
        crypto.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(),
      /unavailable in browser-only crypto builds/,
      `${label} recursive aggregation prover must be native-only`,
    );
    assert.throws(
      () =>
        crypto.kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(),
      /unavailable in browser-only crypto builds/,
      `${label} recursive compact prover must be native-only`,
    );
    assert.equal(crypto.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1, 1);
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
      8 * 1024 * 1024,
    );
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES,
      128,
    );
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN,
      "iroha:kagemusha:v1:recursive-spend-transition-profile",
    );
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN,
      "iroha:kagemusha:v1:recursive-spend-transition-profile-digest",
    );
    assert.equal(
      crypto.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN,
      "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest",
    );
    assert.equal(
      crypto.normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(""),
      crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    );
    assert.equal(
      crypto.isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(""),
      true,
    );
    assert.equal(
      crypto.isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
        crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      ),
      true,
    );
    assert.equal(
      crypto.requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
        crypto.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
      ),
      false,
    );
    assert.equal(
      crypto.requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend("", 1),
      false,
    );
    assert.equal(crypto.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES, 64 * 1024 * 1024);
    assert.equal(crypto.PRIVACY_FFI_VERSION_V1, 1);
    assert.equal(crypto.PRIVACY_REQUIRED_BRIDGE_ABI_VERSION, 6);
    assert.equal(crypto.PRIVACY_FFI_STATUS_ERROR, 1);
    assert.equal(crypto.PRIVACY_FFI_ERROR_NULL_POINTER, 1);
    assert.equal(crypto.PRIVACY_FFI_ERROR_MALFORMED_NORITO, 2);
    assert.equal(crypto.PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM, 3);
    assert.equal(crypto.PRIVACY_FFI_ERROR_PRODUCTION_DISABLED, 4);
    assert.equal(crypto.PRIVACY_FFI_ERROR_INVALID_REQUEST, 5);
    assert.throws(
      () => crypto.privacyCapabilitiesV1(),
      /privacyCapabilitiesV1 is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.privacyBuildProofV1(Buffer.from([1])),
      /privacyBuildProofV1 is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.privacyVerifyProofV1(Buffer.from([1])),
      /privacyVerifyProofV1 is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.kagemushaRecursiveSpendInit(Buffer.from([1])),
      /kagemushaRecursiveSpendInit is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.kagemushaRecursiveSpendTransitionProfileInit(Buffer.from([1])),
      /kagemushaRecursiveSpendTransitionProfileInit is unavailable in browser-only crypto builds/,
    );
    assert.throws(
      () => crypto.kagemushaRecursiveSpendTransitionProfileAppend(Buffer.from([1])),
      /kagemushaRecursiveSpendTransitionProfileAppend is unavailable in browser-only crypto builds/,
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

test("browser package wiring keeps privacy catalogs on mapped crypto stubs", () => {
  const packageJson = JSON.parse(
    readFileSync(new URL("../package.json", import.meta.url), "utf8"),
  );
  assert.equal(packageJson.exports["./crypto"].browser, "./dist/crypto.browser.js");
  assert.equal(packageJson.browser["./dist/crypto.js"], "./dist/crypto.browser.js");

  for (const [label, relativePath] of [
    ["src", "../src/privacyAlgorithms.js"],
    ["dist", "../dist/privacyAlgorithms.js"],
  ]) {
    const source = readFileSync(new URL(relativePath, import.meta.url), "utf8");
    assert.match(
      source,
      /^import \{ isPrivacyNativeAvailable \} from "\.\/crypto\.js";/m,
      `${label} privacy catalog must route bridge detection through browser-mapped crypto.js`,
    );
    assert.doesNotMatch(
      source,
      /iroha_js_host|__IROHA_NATIVE_BINDING__|privacyCapabilitiesV1|privacyBuildProofV1|privacyVerifyProofV1/,
      `${label} privacy catalog must not call native privacy FFI directly`,
    );
  }
});
