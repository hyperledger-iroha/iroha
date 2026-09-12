import assert from "node:assert/strict";
import test from "node:test";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { acquireDistLock, buildDistribution, releaseDistLock } from "../scripts/build-dist.mjs";
import { publishBrowserCodecGeneration } from "../scripts/publish-browser-codec.mjs";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "iroha-wasm-generation-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  return root;
}

function generation(directory, marker) {
  mkdirSync(directory, { recursive: true });
  // Publication fixtures only; these bytes do not claim codec qualification.
  writeFileSync(join(directory, "iroha_js_codec_wasm.js"), `// generation ${marker}\n`);
  writeFileSync(join(directory, "iroha_js_codec_wasm.d.ts"), `// generation ${marker}\n`);
  writeFileSync(join(directory, "iroha_js_codec_wasm_bg.wasm"), Buffer.from([0, 97, 115, 109, 1, 0, 0, 0, marker]));
  writeFileSync(join(directory, "iroha_js_codec_wasm_bg.wasm.d.ts"), `// generation ${marker}\n`);
}

test("Wasm publication waits for the actual dist reader lock and replaces a complete generation", async (t) => {
  const root = fixture(t);
  const output = join(root, "wasm");
  const staging = join(root, ".wasm-stage-test/wasm");
  generation(output, 1);
  generation(staging, 2);
  const lock = await acquireDistLock({ root });
  let completed = false;
  const publishing = publishBrowserCodecGeneration({ staging, output }).then(() => { completed = true; });
  await new Promise((resolve) => setTimeout(resolve, 80));
  assert.equal(completed, false);
  assert.equal(readFileSync(join(output, "iroha_js_codec_wasm_bg.wasm")).at(-1), 1);
  releaseDistLock(lock);
  await publishing;
  assert.equal(readFileSync(join(output, "iroha_js_codec_wasm_bg.wasm")).at(-1), 2);
  assert.equal(existsSync(staging), false);
  assert.equal(existsSync(`${output}.previous`), false);
});

test("concurrent source distribution and Wasm publisher cannot mix glue and binary generations", async (t) => {
  const root = fixture(t);
  cpSync(fileURLToPath(new URL("../src", import.meta.url)), join(root, "src"), { recursive: true });
  const output = join(root, "wasm");
  const staging = join(root, ".wasm-stage-test/wasm");
  generation(output, 1);
  generation(staging, 2);
  await Promise.all([buildDistribution({ root }), publishBrowserCodecGeneration({ staging, output })]);
  const marker = readFileSync(join(root, "dist/wasm/iroha_js_codec_wasm_bg.wasm")).at(-1);
  assert.match(readFileSync(join(root, "dist/wasm/iroha_js_codec_wasm.js"), "utf8"), new RegExp(`generation ${marker}`));
  assert.ok(marker === 1 || marker === 2);
});

test("unknown or symlinked generations and unresolved backups remain untouched", async (t) => {
  const root = fixture(t);
  const output = join(root, "wasm");
  const staging = join(root, ".wasm-stage-test/wasm");
  generation(output, 1);
  generation(staging, 2);
  writeFileSync(join(output, "operator-file"), "retain");
  await assert.rejects(publishBrowserCodecGeneration({ staging, output }), /exactly the four/);
  assert.equal(readFileSync(join(output, "operator-file"), "utf8"), "retain");
  rmSync(join(output, "operator-file"));
  const link = join(staging, "iroha_js_codec_wasm.d.ts");
  rmSync(link);
  symlinkSync(join(output, "iroha_js_codec_wasm.d.ts"), link);
  await assert.rejects(publishBrowserCodecGeneration({ staging, output }), /unsupported file/);
  rmSync(link);
  writeFileSync(link, "// restored fixture");
  mkdirSync(`${output}.previous`);
  await assert.rejects(publishBrowserCodecGeneration({ staging, output }), /requires recovery/);
  assert.equal(readFileSync(join(output, "iroha_js_codec_wasm_bg.wasm")).at(-1), 1);
  assert.equal(readFileSync(join(staging, "iroha_js_codec_wasm_bg.wasm")).at(-1), 2);
});
