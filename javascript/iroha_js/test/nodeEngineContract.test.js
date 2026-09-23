// Metadata and synthetic runtime comparisons only; no SDK/native imports.
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { validateNodeEngineContract } from "../scripts/node-engine-contract.mjs";

const pkg = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const lock = JSON.parse(readFileSync(new URL("../package-lock.json", import.meta.url), "utf8"));
const hashes = "node_modules/@scure/bip39/node_modules/@noble/hashes";
function copies() { return [structuredClone(pkg), structuredClone(lock)]; }

test("published exact floor covers all nine locked production dependency locations", () => {
  assert.equal(pkg.engines.node, ">=20.19.0");
  assert.equal(lock.packages[""].engines.node, ">=20.19.0");
  assert.equal(lock.packages[hashes].engines.node, ">= 20.19.0");
  for (const runtime of ["20.19.0", "20.20.0", "22.0.0", "24.0.0"]) {
    assert.deepEqual(validateNodeEngineContract(pkg, lock, runtime), {
      minimum: "20.19.0", productionDependencies: 9,
    });
  }
});

for (const runtime of ["18.20.8", "20.0.0", "20.18.9", "20.19.0-rc.1", "020.19.0", "v20.19.0", "20.19", "9007199254740992.0.0"]) {
  test(`current runtime ${runtime} cannot pass the floor`, () => {
    assert.throws(() => validateNodeEngineContract(pkg, lock, runtime), /below SDK|version spelling|exact integer/u);
  });
}

for (const [name, mutate, error] of [
  ["root lowered with matching lock", (p, l) => { p.engines.node = l.packages[""].engines.node = ">=20.18.0"; }, /outside published/u],
  ["root advertises old major", (p, l) => { p.engines.node = l.packages[""].engines.node = ">=18"; }, /canonical/u],
  ["root omits floor", (p) => { delete p.engines.node; }, /declare/u],
  ["lock floor differs", (_p, l) => { l.packages[""].engines.node = ">=20.20.0"; }, /contracts differ/u],
  ["transitive dependency raises minimum", (_p, l) => { l.packages[hashes].engines.node = ">=22.0.0"; }, /outside published/u],
  ["transitive dependency hides as dev", (_p, l) => { l.packages[hashes].dev = true; }, /development-only/u],
  ["transitive dependency disappears", (_p, l) => { delete l.packages[hashes]; }, /does not match production selection/u],
  ["wrong locked production version", (_p, l) => { l.packages[hashes].version = "1.8.0"; }, /does not match production selection/u],
  ["unreviewed selection grammar", (_p, l) => { l.packages["node_modules/@scure/bip39"].dependencies["@noble/hashes"] = "*"; }, /version spelling/u],
  ["non-tail dependency range", (_p, l) => { l.packages[hashes].engines.node = "^20.19.0"; }, /outside published/u],
  ["unknown comparator grammar", (_p, l) => { l.packages[hashes].engines.node = ">=20.19.0 <21"; }, /unreviewed/u],
  ["malformed engine scalar", (_p, l) => { l.packages[hashes].engines = true; }, /engines must be an object/u],
  ["malformed engine node", (_p, l) => { l.packages[hashes].engines.node = true; }, /must be a string/u],
  ["missing production selection", (_p, l) => { delete l.packages[""].dependencies.buffer; }, /selections differ/u],
  ["unowned production row", (_p, l) => { l.packages["node_modules/unowned"] = { version: "1.0.0" }; }, /unowned production/u],
  ["unreviewed peer relation", (_p, l) => { l.packages[hashes].peerDependencies = { unknown: "1.0.0" }; }, /unreviewed peerDependencies/u],
  ["foreign lock layout", (_p, l) => { l.lockfileVersion = 2; }, /package-lock V3/u],
]) {
  test(`publishing rejects ${name}`, () => {
    const [p, l] = copies(); mutate(p, l);
    assert.throws(() => validateNodeEngineContract(p, l, "24.0.0"), error);
  });
}

test("development-only tool minimum does not alter the published runtime closure", () => {
  const [p, l] = copies();
  l.packages["node_modules/typescript"].engines.node = ">=999.0.0";
  assert.equal(validateNodeEngineContract(p, l, "20.19.0").productionDependencies, 9);
});

test("a scoped nested dependency is checked even when the hoisted version accepts older Node", () => {
  assert.equal(lock.packages["node_modules/@noble/hashes"].engines.node, "^14.21.3 || >=16");
  const [p, l] = copies();
  p.engines.node = l.packages[""].engines.node = ">=20.18.9";
  assert.throws(() => validateNodeEngineContract(p, l, "24.0.0"), new RegExp(hashes + " requires", "u"));
});

test("publish and pack execute the guard before build or release actions", () => {
  assert.equal(pkg.scripts["check:node-engine"], "node ./scripts/check-node-engine.mjs");
  assert.equal(pkg.scripts.prepack, "npm run check:node-engine && npm run build:dist");
  assert.ok(pkg.scripts.prepublishOnly.startsWith("npm run check:node-engine && npm run check:changelog"));
  const bundle = readFileSync(new URL("../scripts/bundle-size-check.mjs", import.meta.url), "utf8");
  assert.match(bundle, /platform: "node",\s*target: "node20\.19"/u);
});

test("the fixed CLI reads its package and lock without SDK/native loading", () => {
  const result = spawnSync(process.execPath, [fileURLToPath(new URL("../scripts/check-node-engine.mjs", import.meta.url))], {
    encoding: "utf8", env: {}, timeout: 10_000,
  });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^Node >=20\.19\.0 covers 9 locked production dependencies; current /u);
  const unexpected = spawnSync(process.execPath, [fileURLToPath(new URL("../scripts/check-node-engine.mjs", import.meta.url)), "--skip"], {
    encoding: "utf8", env: {}, timeout: 10_000,
  });
  assert.equal(unexpected.status, 1);
  assert.match(unexpected.stderr, /takes no options/u);
});

// Copy only the current guard and inert metadata into a fresh target-owned
// directory. No package scripts, SDK modules, dependencies or addons execute.
function cliFixture(t, floor) {
  const target = fileURLToPath(new URL("../../../target/", import.meta.url));
  mkdirSync(target, { recursive: true });
  const directory = mkdtempSync(join(target, "node-engine-contract-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const original = join(directory, "sdk");
  const decoy = join(directory, "decoy");
  function metadata(root, minimum) {
    mkdirSync(join(root, "scripts"), { recursive: true });
    const inert = { name: "inert-node-floor", version: "1.0.0", engines: { node: `>=${minimum}` } };
    writeFileSync(join(root, "package.json"), JSON.stringify(inert));
    writeFileSync(join(root, "package-lock.json"), JSON.stringify({ lockfileVersion: 3, packages: { "": inert } }));
  }
  metadata(original, floor);
  // A file alias with --preserve-symlinks-main must still read the real guard's
  // metadata, not this lower-floor metadata adjacent to its apparent pathname.
  metadata(decoy, "20.19.0");
  const script = join(original, "scripts", "check-node-engine.mjs");
  writeFileSync(script, readFileSync(new URL("../scripts/check-node-engine.mjs", import.meta.url)));
  writeFileSync(join(original, "scripts", "node-engine-contract.mjs"),
    readFileSync(new URL("../scripts/node-engine-contract.mjs", import.meta.url)));
  const fileAlias = join(decoy, "scripts", "check-node-engine.mjs");
  symlinkSync(script, fileAlias, "file");
  const directoryAlias = join(directory, "sdk-link");
  symlinkSync(original, directoryAlias, process.platform === "win32" ? "junction" : "dir");
  return {
    directory,
    paths: {
      normal: script,
      "file symlink": fileAlias,
      "ancestor directory symlink": join(directoryAlias, "scripts", "check-node-engine.mjs"),
    },
  };
}

for (const entry of ["normal", "file symlink", "ancestor directory symlink"]) {
  for (const preserve of [false, true]) {
    const flags = preserve ? ["--preserve-symlinks-main"] : [];
    const label = `${entry}, preserve-symlinks-main=${preserve}`;
    for (const scenario of ["normal floor", "raised floor", "unexpected option"]) {
      test(`CLI ${label} enforces ${scenario}`, (t) => {
        const f = cliFixture(t, scenario === "raised floor" ? "99.0.0" : "20.19.0");
        const args = scenario === "unexpected option" ? ["--skip"] : [];
        const result = spawnSync(process.execPath, [...flags, f.paths[entry], ...args], {
          cwd: f.directory, encoding: "utf8", env: {}, timeout: 10_000, maxBuffer: 65_536,
        });
        assert.equal(result.error, undefined);
        assert.equal(result.signal, null);
        if (scenario === "normal floor") {
          assert.equal(result.status, 0, result.stderr);
          assert.match(result.stdout, /^Node >=20\.19\.0 covers 0 locked production dependencies; current /u);
          assert.equal(result.stderr, "");
        } else {
          assert.equal(result.status, 1, `${result.stdout}\n${result.stderr}`);
          assert.equal(result.stdout, "");
          assert.match(result.stderr, scenario === "raised floor" ? /below SDK >=99\.0\.0/u : /takes no options/u);
        }
      });
    }
  }
}

for (const inputMode of ["stdin", "eval"]) {
  for (const scriptArgs of [[], ["arbitrary-nonexistent-file", "--skip"]]) {
    test(`pure guard import from ${inputMode}, arbitrary argv=${scriptArgs.length > 0}`, () => {
      const url = new URL("../scripts/node-engine-contract.mjs", import.meta.url).href;
      const source = `import { validateNodeEngineContract } from ${JSON.stringify(url)}; console.log(typeof validateNodeEngineContract);`;
      const args = inputMode === "stdin" ? ["--input-type=module", "-", ...scriptArgs] : ["--input-type=module", "--eval", source, "--", ...scriptArgs];
      const result = spawnSync(process.execPath, args, {
        input: inputMode === "stdin" ? source : undefined,
        encoding: "utf8", env: {}, timeout: 10_000, maxBuffer: 65_536,
    });
    assert.equal(result.error, undefined);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout, "function\n");
    assert.equal(result.stderr, "");
    });
  }
}

for (const location of [
  "node_modules/@noble/node_modules/@noble/hashes",
  "node_modules/@scure/node_modules/@scure/base",
  "node_modules//hidden",
  "node_modules/../hidden",
  "node_modules/hidden/",
  "node_modules/hidden/lib/node_modules/other",
  "node_modules\\hidden",
]) {
  test(`an unreviewed development-only lock location cannot hide an engine: ${location}`, () => {
    const [p, l] = copies();
    l.packages[location] = { version: "1.8.0", engines: { node: ">=99.0.0" }, dev: true };
    assert.throws(() => validateNodeEngineContract(p, l, "20.19.0"), /unreviewed lock package location/u);
  });
}
