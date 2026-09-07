import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";
import { buildDistribution } from "../scripts/build-dist.mjs";
import { findForbiddenBrowserInputs } from "../scripts/bundle-size-check.mjs";

test("isolated packed Touring browser entry executes all native fixtures with strict DOM types", async () => {
  const sdkRoot = fileURLToPath(new URL("..", import.meta.url));
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "iroha-classed-package-"));
  try {
    const source = path.join(work, "source"); fs.mkdirSync(source);
    fs.cpSync(path.join(sdkRoot, "src"), path.join(source, "src"), { recursive: true });
    for (const name of fs.readdirSync(sdkRoot)) {
      if (name.endsWith(".d.ts") || ["package.json", "README.md", "LICENSE"].includes(name)) {
        fs.copyFileSync(path.join(sdkRoot, name), path.join(source, name));
      }
    }
    await buildDistribution({ root: source });
    const packed = spawnSync("npm", ["pack", "--ignore-scripts", "--json", "--pack-destination", work], {
      cwd: source, encoding: "utf8",
    });
    assert.equal(packed.status, 0, packed.stdout + packed.stderr);
    const archive = path.join(work, JSON.parse(packed.stdout)[0].filename);
    const extracted = spawnSync("tar", ["-xzf", archive, "-C", work], { encoding: "utf8" });
    assert.equal(extracted.status, 0, extracted.stderr);
    const installed = path.join(work, "node_modules", "@iroha", "iroha-js");
    fs.mkdirSync(path.dirname(installed), { recursive: true });
    fs.renameSync(path.join(work, "package"), installed);
    const manifest = JSON.parse(fs.readFileSync(path.join(installed, "package.json"), "utf8"));
    assert.deepEqual(manifest.exports["./classed-race"], {
      types: "./classed-race.d.ts", browser: "./dist/classedRace.js", import: "./dist/classedRace.js",
    });
    assert.deepEqual(manifest.typesVersions["*"]["classed-race"], ["./classed-race.d.ts"]);
    assert.ok(manifest.files.includes("classed-race.d.ts"));
    const entry = path.join(work, "entry.mjs");
    fs.writeFileSync(entry, 'export * from "@iroha/iroha-js/classed-race";\n');
    const bundle = await build({
      entryPoints: [entry], absWorkingDir: work, nodePaths: [path.join(sdkRoot, "node_modules")],
      bundle: true, platform: "browser", target: "es2020", format: "esm", minify: true,
      write: false, treeShaking: true, metafile: true,
    });
    const inputs = Object.keys(bundle.metafile.inputs);
    assert.deepEqual(findForbiddenBrowserInputs(inputs), []);
    assert.ok(inputs.some(input => /dist[/\\]classedRace\.js$/.test(input)));
    assert.ok(inputs.some(input => /dist[/\\]native\.browser\.js$/.test(input)));
    assert.doesNotMatch(bundle.outputFiles[0].text, /(?:globalThis|window|global)\.Buffer\s*=/);
    const browser = await import(`data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].contents).toString("base64")}`);
    const fixtures = JSON.parse(fs.readFileSync(new URL("fixtures/classed-race-v1-codec.json", import.meta.url), "utf8"));
    for (const row of fixtures.vectors) {
      const bare = browser.encodeClassedRaceValueV1(row.name, row.value);
      assert.equal(Buffer.from(bare).toString("hex").toUpperCase(), row.encoded_hex);
      const decoded = browser.decodeClassedRaceFrameV1(row.name, Uint8Array.from(Buffer.from(row.framed_hex, "hex")));
      assert.equal(Buffer.from(browser.encodeClassedRaceValueV1(row.name, decoded)).toString("hex").toUpperCase(), row.encoded_hex);
    }
    const declarations = fs.readFileSync(path.join(installed, "classed-race.d.ts"), "utf8");
    assert.doesNotMatch(declarations, /reference types=["']node|from ["']node:/);
    fs.writeFileSync(path.join(work, "consumer.mts"), [
      'import { encodeClassedRaceValueV1, decodeClassedRaceValueV1, decodeClassedRaceFrameV1, type ClassedRaceReplayV1 } from "@iroha/iroha-js/classed-race";',
      'const replay: ClassedRaceReplayV1 = { version: 1, class_id: { kind: "touring_s1", value: null }, track: { kind: "sakura", value: null }, player_count: 1, frames: [], dnf_events: [] };',
      'const bytes: Uint8Array = encodeClassedRaceValueV1("ClassedRaceReplayV1", replay);',
      'const decoded: ClassedRaceReplayV1 = decodeClassedRaceValueV1("ClassedRaceReplayV1", bytes);',
      'const framed: ClassedRaceReplayV1 = decodeClassedRaceFrameV1("ClassedRaceReplayV1", bytes);',
      'void decoded; void framed;',
      '// @ts-expect-error no stock or instruction fallback',
      'encodeClassedRaceValueV1("RaceReplayV1", replay);',
      '// @ts-expect-error only the compiled Touring class exists',
      'encodeClassedRaceValueV1("ClassedRaceClassV1", { kind: "stock", value: null });',
      '// @ts-expect-error required explicit optional timestamps',
      'encodeClassedRaceValueV1("ClassedRaceStandingV1", { slot: 0, progress_mm: 0 });',
    ].join("\n"));
    fs.writeFileSync(path.join(work, "tsconfig.json"), JSON.stringify({
      compilerOptions: { strict: true, noEmit: true, target: "ES2020", module: "NodeNext",
        moduleResolution: "NodeNext", lib: ["ES2020", "DOM"], types: [], skipLibCheck: false },
      files: ["consumer.mts"],
    }));
    const typed = spawnSync(process.execPath, [path.join(sdkRoot, "node_modules/typescript/bin/tsc"), "-p", work], { encoding: "utf8" });
    assert.equal(typed.status, 0, typed.stdout + typed.stderr);
    console.log(JSON.stringify({ scope: "packed browser-target bundle executed in Node ESM; not Chromium or native proof verification",
      vectors: fixtures.vectors.length, bundle_bytes: bundle.outputFiles[0].contents.byteLength,
      bundle_sha256: createHash("sha256").update(bundle.outputFiles[0].contents).digest("hex"),
      strict_dom_types: true, forbidden_browser_inputs: [] }));
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
});
