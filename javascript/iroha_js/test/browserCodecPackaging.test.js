import assert from "node:assert/strict";
import test from "node:test";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { buildDistribution, directoryDigest } from "../scripts/build-dist.mjs";
import { verifyBrowserCodec } from "../scripts/verify-browser-codec.mjs";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "iroha-browser-codec-publication-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  cpSync(fileURLToPath(new URL("../src", import.meta.url)), join(root, "src"), { recursive: true });
  return root;
}

test("source-only distribution needs no Wasm build but cannot qualify as a package", async (t) => {
  const root = fixture(t);
  await buildDistribution({ root });
  assert.equal(existsSync(join(root, "dist/wasm")), false);
  await assert.rejects(verifyBrowserCodec(join(root, "dist")), { code: "ENOENT" });
});

test("partial generated artifacts fail without replacing the current distribution", async (t) => {
  const root = fixture(t);
  await buildDistribution({ root });
  const before = directoryDigest(join(root, "dist"));
  mkdirSync(join(root, "wasm"));
  writeFileSync(join(root, "wasm/iroha_js_codec_wasm.js"), "// incomplete packaging fixture\n");
  await assert.rejects(buildDistribution({ root }), /artifact is missing/);
  assert.equal(directoryDigest(join(root, "dist")), before);
});

test("atomic distribution carries both generated assets while package verification rejects fake glue", async (t) => {
  const root = fixture(t);
  mkdirSync(join(root, "wasm"));
  // This is only a file-publication fixture, never a codec/parity fixture.
  writeFileSync(join(root, "package.json"), '{"type":"module"}\n');
  writeFileSync(join(root, "wasm/iroha_js_codec_wasm.js"), "export default async function init() {}\n");
  const emptyModule = Uint8Array.of(0, 97, 115, 109, 1, 0, 0, 0);
  writeFileSync(join(root, "wasm/iroha_js_codec_wasm_bg.wasm"), emptyModule);
  await buildDistribution({ root });
  assert.deepEqual(readFileSync(join(root, "dist/wasm/iroha_js_codec_wasm_bg.wasm")), Buffer.from(emptyModule));
  assert.equal(directoryDigest(join(root, "src")), directoryDigest(join(root, "dist"), { excludeGeneratedBrowserCodec: true }));
  await assert.rejects(verifyBrowserCodec(join(root, "dist")), { code: "ERR_IROHA_CODEC_INITIALIZATION" });
});
