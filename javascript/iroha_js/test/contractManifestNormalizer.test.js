import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { buildRegisterSmartContractCodeInstruction } from "../src/instructionBuilders.js";
import { noritoDecodeInstruction, noritoEncodeInstruction } from "../src/norito.js";

const fixture = JSON.parse(
  readFileSync(new URL("./fixtures/contract_manifest_v1.json", import.meta.url), "utf8"),
);

function manifestFixture() {
  return structuredClone(fixture.manifest);
}

function observe(object, key, events, label = key, value = object[key]) {
  Object.defineProperty(object, key, {
    enumerable: true,
    configurable: true,
    get() {
      events.push(label);
      return value;
    },
  });
}

test("public manifest builder preserves the Rust fixture and canonical instruction bytes", () => {
  const manifest = manifestFixture();
  const instruction = buildRegisterSmartContractCodeInstruction({ manifest });
  assert.deepEqual(instruction, { RegisterSmartContractCode: { manifest: fixture.manifest } });
  assert.deepEqual(manifest, fixture.manifest);
  assert.notEqual(instruction.RegisterSmartContractCode.manifest, manifest);
  assert.notEqual(instruction.RegisterSmartContractCode.manifest.entrypoints, manifest.entrypoints);

  const encoded = noritoEncodeInstruction(instruction, 753);
  const rustManifest = Buffer.from(fixture.manifest_compact_hex, "hex");
  assert.notEqual(encoded.indexOf(rustManifest), -1, "instruction must contain exact Rust manifest bytes");
  const decoded = noritoDecodeInstruction(encoded, 753);
  assert.deepEqual(decoded, instruction);
  assert.deepEqual(noritoEncodeInstruction(decoded, 753), encoded);
});

test("entrypoint getters retain their validation order", () => {
  const manifest = manifestFixture();
  const entrypoint = manifest.entrypoints[0];
  const events = [];
  const fields = [
    "name", "permission", "kind", "params", "argument_schema", "return_type",
    "return_schema", "read_keys", "write_keys", "access_hints_complete",
    "access_hints_skipped", "triggers",
  ];
  for (const field of fields) observe(entrypoint, field, events);
  const instruction = buildRegisterSmartContractCodeInstruction({ manifest });
  assert.deepEqual(events, fields);
  assert.deepEqual(instruction.RegisterSmartContractCode.manifest, fixture.manifest);
});

test("an entrypoint getter failure stops before later entrypoint fields", () => {
  const manifest = manifestFixture();
  const entrypoint = manifest.entrypoints[0];
  const events = [];
  const failure = new Error("permission getter failed");
  observe(entrypoint, "name", events);
  Object.defineProperty(entrypoint, "permission", {
    get() {
      events.push("permission");
      throw failure;
    },
  });
  observe(entrypoint, "kind", events);
  observe(entrypoint, "params", events);
  assert.throws(() => buildRegisterSmartContractCodeInstruction({ manifest }), error => error === failure);
  assert.deepEqual(events, ["name", "permission"]);
});

test("invalid trigger metadata fails after callback admission and before trigger fields", () => {
  const manifest = manifestFixture();
  const trigger = manifest.entrypoints[0].triggers[0];
  const events = [];
  observe(trigger, "callback", events);
  observe(trigger, "metadata", events, "metadata", []);
  observe(trigger, "id", events);
  observe(trigger, "repeats", events);
  assert.throws(() => buildRegisterSmartContractCodeInstruction({ manifest }), {
    code: "ERR_INVALID_OBJECT",
    path: "manifest.entrypoints[0].triggers[0].metadata",
    message: "manifest.entrypoints[0].triggers[0].metadata must be an object",
  });
  assert.deepEqual(events, ["callback", "metadata"]);
});

test("trigger metadata enumeration precedes value validation and callback field reads", () => {
  const manifest = manifestFixture();
  const trigger = manifest.entrypoints[0].triggers[0];
  const events = [];
  const metadata = {};
  observe(metadata, "first", events, "metadata.first", Number.NaN);
  observe(metadata, "second", events, "metadata.second", "read before first is validated");
  observe(trigger, "callback", events);
  observe(trigger, "metadata", events, "metadata", metadata);
  observe(trigger, "id", events);
  observe(trigger, "repeats", events);
  observe(trigger, "filter", events);
  observe(trigger, "authority", events);
  observe(trigger.callback, "namespace", events, "callback.namespace");
  observe(trigger.callback, "entrypoint", events, "callback.entrypoint");
  events.length = 0;
  assert.throws(() => buildRegisterSmartContractCodeInstruction({ manifest }), {
    code: "ERR_INVALID_JSON_VALUE",
    path: "manifest.entrypoints[0].triggers[0].metadata.first",
    message: "manifest.entrypoints[0].triggers[0].metadata.first must not contain non-finite numbers",
  });
  assert.deepEqual(events, [
    "callback", "metadata", "id", "repeats", "filter", "authority", "authority",
    "metadata.first", "metadata.second",
  ]);
});

for (const surface of ["provenance", "authority"]) {
  test(`valid manifest ${surface} requires the real native account owner`, () => {
    const directory = mkdtempSync(join(tmpdir(), "iroha-manifest-native-absence-"));
    try {
      const script = `
        import assert from 'node:assert/strict';
        import { readFileSync } from 'node:fs';
        import { buildRegisterSmartContractCodeInstruction } from './src/instructionBuilders.js';
        const fixture = JSON.parse(readFileSync('./test/fixtures/contract_manifest_v1.json', 'utf8'));
        const manifest = structuredClone(fixture.manifest);
        if (${JSON.stringify(surface)} === 'provenance') {
          manifest.provenance = fixture.signed_provenance;
        } else {
          const accounts = JSON.parse(readFileSync('../../fixtures/account/multisig_wire_v1.json', 'utf8'));
          const account = accounts.positive.find(entry => entry.policy !== null);
          assert(account, 'Rust fixture must contain a complete multisig controller');
          manifest.entrypoints[0].triggers[0].authority = account.i105;
        }
        assert.throws(() => buildRegisterSmartContractCodeInstruction({manifest}), {
          code: 'ERR_IROHA_NATIVE_BINDING', nativeStatus: 'missing_file',
        });
      `;
      const child = spawnSync(process.execPath, ["--input-type=module", "--eval", script], {
        cwd: fileURLToPath(new URL("..", import.meta.url)),
        env: { ...process.env, IROHA_JS_NATIVE_DIR: join(directory, "absent-native") },
        encoding: "utf8",
      });
      assert.equal(child.error, undefined);
      assert.equal(child.signal, null);
      assert.equal(child.status, 0, child.stderr || child.stdout);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
}
