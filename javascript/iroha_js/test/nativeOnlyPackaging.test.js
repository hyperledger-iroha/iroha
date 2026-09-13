import assert from "node:assert/strict";
import test from "node:test";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { buildDistribution, directoryDigest } from "../scripts/build-dist.mjs";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "iroha-native-only-publication-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  cpSync(fileURLToPath(new URL("../src", import.meta.url)), join(root, "src"), { recursive: true });
  return root;
}

test("distribution publishes the complete source tree without generated codecs", async (t) => {
  const root = fixture(t);
  await buildDistribution({ root });
  assert.equal(directoryDigest(join(root, "src")), directoryDigest(join(root, "dist")));
  for (const retired of ["wasm", "browserCodec.js", "browserCodecRuntime.js", "public/browserCodec.js"]) {
    assert.equal(existsSync(join(root, "dist", retired)), false);
  }
});

test("distribution replaces stale generated codecs without importing package-side artifacts", async (t) => {
  const root = fixture(t);
  await buildDistribution({ root });
  mkdirSync(join(root, "dist/wasm"));
  mkdirSync(join(root, "wasm"));
  writeFileSync(join(root, "dist/wasm/old.wasm"), "stale distribution asset");
  writeFileSync(join(root, "wasm/old.wasm"), "unpublished package-side asset");
  assert.equal((await buildDistribution({ root })).changed, true);
  assert.equal(existsSync(join(root, "dist/wasm")), false);
  assert.equal(directoryDigest(join(root, "src")), directoryDigest(join(root, "dist")));
});

test("package exposes no browser codec initialization or build path", () => {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  assert.equal(Object.hasOwn(manifest.exports, "./browser-codec"), false);
  assert.equal(Object.hasOwn(manifest.typesVersions["*"], "browser-codec"), false);
  assert.equal(manifest.files.includes("browser-codec.d.ts"), false);
  assert.equal(Object.hasOwn(manifest.scripts, "build:wasm"), false);
  assert.equal(Object.hasOwn(manifest.scripts, "verify:browser-codec"), false);
  assert.equal(manifest.scripts.prepack, "npm run build:dist");
  for (const retired of [
    "src/browserCodec.js", "src/browserCodecRuntime.js", "src/public/browserCodec.js",
    "browser-codec.d.ts", "scripts/build-wasm.py", "scripts/verify-browser-codec.mjs",
    "scripts/publish-browser-codec.mjs",
  ]) assert.equal(existsSync(join(root, retired)), false, retired);
});
