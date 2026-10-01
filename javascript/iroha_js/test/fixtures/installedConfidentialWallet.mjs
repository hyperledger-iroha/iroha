// Copied into an isolated npm consumer; every SDK import resolves from its installed package.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, realpathSync } from "node:fs";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as sdk from "@iroha/iroha-js";
import { confidentialWalletNativeCases } from "./wallet-cases.mjs";

const sha = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");
const [expectedNativeHash, expectedManifestHash] = process.argv.slice(2);
assert.match(expectedNativeHash, /^[a-f0-9]{64}$/u);
assert.match(expectedManifestHash, /^[a-f0-9]{64}$/u);
const sdkEntry = realpathSync(fileURLToPath(import.meta.resolve("@iroha/iroha-js")));
assert.equal(sdkEntry, realpathSync(join(process.cwd(), "node_modules/@iroha/iroha-js/dist/index.js")));
const native = join(process.env.IROHA_JS_NATIVE_DIR, "iroha_js_host.node");
const manifest = join(process.env.IROHA_JS_NATIVE_DIR, "iroha_js_host.checksums.json");
assert.equal(sha(native), expectedNativeHash);
assert.equal(sha(manifest), expectedManifestHash);
const cases = [];
for (const [name, run] of confidentialWalletNativeCases(sdk)) {
  cases.push({ name, passed: true, result: await run() });
}
assert.equal(cases.length, 2);
// The ordinary loader materializes and loads its immutable verified image.
// Bind that actual process-reported image, not merely an environment search path.
const loaded = process.report.getReport().sharedObjects.filter((path) => basename(path) === `${expectedNativeHash}.node`);
assert.equal(loaded.length, 1, "Exactly one authenticated loaded native image is required");
assert.equal(sha(loaded[0]), expectedNativeHash);
assert.equal(sha(native), expectedNativeHash);
assert.equal(sha(manifest), expectedManifestHash);
console.log(JSON.stringify({
  kind: "installed-confidential-wallet", sdkEntry, sdkEntrySha256: sha(sdkEntry),
  nativeSha256: expectedNativeHash, nativeManifestSha256: expectedManifestHash,
  loadedImage: loaded[0], loadedImageSha256: sha(loaded[0]),
  passed: cases.length, failed: 0, skipped: 0, cases,
}));
