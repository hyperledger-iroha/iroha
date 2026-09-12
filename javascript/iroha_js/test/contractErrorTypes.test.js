import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { buildRegisterSmartContractCodeInstruction } from "../src/instructionBuilders.js";
import { ToriiClient } from "../src/toriiClient.js";
import { normalizeContractErrorTypeV1, normalizeContractErrorTypesV1, validateManifestErrorTypeBindingsV1 } from "../src/contractErrorTypes.js";
import { analyzeEntrypointValueTypeV1 } from "../src/entrypointSchema.js";
import { isCanonicalKotodamaStateTypeName, isCanonicalKotodamaStructName } from "../src/kotodamaIdentifiers.js";
import { _createNoritoInstructionApi } from "../src/norito.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";

const error = { identity: "example/vault@1.0.0::金庫::拒否", variants: [{ name: "不足", code: 1 }, { name: "CapacityExceeded", code: 2 }] };
const returnSchema = { nodes: [{ kind: "Result", value: null }, { kind: "Unit", value: null }, { kind: "Error", value: error }] };

test("state-only nominal identities require the exact manifest catalog", async () => {
  const fixture = JSON.parse(await readFile(new URL("../../../fixtures/kotodama/nominal_errors_v1.json", import.meta.url), "utf8"));
  const manifest = { ...fixture.manifest, entrypoints: [] };
  validateManifestErrorTypeBindingsV1(manifest);
  assert.throws(() => validateManifestErrorTypeBindingsV1({ ...manifest, error_types: [] }), /error_types catalog/u);
  for (const typeName of [
    "missing/package@1::Vault::Failure",
    "Result<(), missing/package@1::Vault::Failure>",
    "StateMap<int, List<Option<missing/package@1::Vault::Failure>, 8>>",
    "Record{status: missing/package@1::Vault::Failure}",
  ]) {
    assert.throws(() => validateManifestErrorTypeBindingsV1({
      ...manifest, states: [{ name: "status", type_name: typeName }],
    }), /error_types catalog/u);
  }
});

test("shared SDK nominal fixture binds the same Japanese error and Unit schemas", async () => {
  const fixture = JSON.parse(await readFile(new URL("../../../fixtures/kotodama/nominal_errors_v1.json", import.meta.url), "utf8"));
  validateManifestErrorTypeBindingsV1(fixture.manifest);
  assert.deepEqual(fixture.manifest.error_types[0], error);
  assert.deepEqual(fixture.manifest.entrypoints[0].return_schema, returnSchema);
  const cursor = fixture.manifest.entrypoints[1].return_schema;
  assert.equal(analyzeEntrypointValueTypeV1(cursor).canonicalName, "Option<StateCursor<int>>");
  assert.equal(analyzeEntrypointValueTypeV1(cursor).wordCount, 1);
  assert.equal(isCanonicalKotodamaStateTypeName("Option<StateCursor<int>>"), true);
  assert.equal(isCanonicalKotodamaStateTypeName("StateCursor<Json>"), false);
  const page = fixture.manifest.entrypoints[2].return_schema;
  assert.equal(analyzeEntrypointValueTypeV1(page).canonicalName, "StatePage<int, bool, 8>");
  assert.equal(analyzeEntrypointValueTypeV1(page).wordCount, 2);
  assert.equal(isCanonicalKotodamaStateTypeName(fixture.manifest.states[2].type_name), true);
  const forged = structuredClone(page);
  forged.nodes[6].value.kind = "Bool";
  assert.throws(() => analyzeEntrypointValueTypeV1(forged), /forged StatePage/u);
  assert.throws(() => analyzeEntrypointValueTypeV1({ nodes: [{ kind: "StateCursor", value: { kind: "Json", value: null } }] }), /Json/u);
});

test("nominal catalogs allow enum-local codes and reject malformed or duplicate schemas", () => {
  assert.deepEqual(normalizeContractErrorTypeV1(error), error);
  assert.equal(normalizeContractErrorTypesV1([error, { ...error, identity: "kotodama::ListError" }]).length, 2);
  for (const value of [
    { ...error, identity: "Injected<Error>" },
    { ...error, variants: [{ name: "Rejected", code: 0 }] },
    { ...error, variants: [...error.variants].reverse() },
    { ...error, variants: [{ name: "Duplicate", code: 1 }, { name: "Duplicate", code: 2 }] },
  ]) assert.throws(() => normalizeContractErrorTypeV1(value), TypeError);
  assert.throws(() => normalizeContractErrorTypesV1([error, error]), /duplicate error identity/u);
});

test("Unit and nominal errors compose into exact public and state types", () => {
  assert.equal(analyzeEntrypointValueTypeV1(returnSchema).canonicalName, `Result<(), ${error.identity}>`);
  assert.equal(analyzeEntrypointValueTypeV1(returnSchema).wordCount, 1);
  assert.equal(isCanonicalKotodamaStateTypeName(`Result<(), ${error.identity}>`), true);
  assert.equal(isCanonicalKotodamaStateTypeName(`StateMap<${error.identity}, bool>`), false);
  const manifest = { error_types: [error], entrypoints: [{ return_schema: returnSchema }] };
  validateManifestErrorTypeBindingsV1(manifest);
  assert.throws(() => validateManifestErrorTypeBindingsV1({ ...manifest, error_types: [{ ...error, variants: [{ name: "Different", code: 1 }] }] }), /does not match/u);
});

test("canonical Norito manifest codecs roundtrip Unit and nominal error schemas without native bindings", async () => {
  const fixture = JSON.parse(await readFile(new URL("./fixtures/contract_manifest_v1.json", import.meta.url), "utf8"));
  const manifest = structuredClone(fixture.manifest);
  manifest.error_types = [error];
  manifest.entrypoints[0].return_type = `Result<(), ${error.identity}>`;
  manifest.entrypoints[0].return_schema = returnSchema;
  const unavailable = { noritoEncodeInstruction() { throw new Error("Native binding required"); }, noritoDecodeInstruction() { throw new Error("Native binding required"); } };
  const api = _createNoritoInstructionApi(createNativeRuntime(unavailable));
  const instruction = { RegisterSmartContractCode: { manifest } };
  const encoded = api.noritoEncodeInstruction(instruction, 753);
  const decoded = api.noritoDecodeInstruction(encoded, 753);
  assert.deepEqual(decoded.RegisterSmartContractCode.manifest.error_types, [error]);
  assert.deepEqual(decoded.RegisterSmartContractCode.manifest.entrypoints[0].return_schema, returnSchema);
  assert.deepEqual(api.noritoEncodeInstruction(decoded, 753), encoded);
});

test("canonical Norito manifest codec preserves nominal state cursor key schemas", async () => {
  const fixture = JSON.parse(await readFile(new URL("./fixtures/contract_manifest_v1.json", import.meta.url), "utf8"));
  const manifest = structuredClone(fixture.manifest);
  const schema = { nodes: [{ kind: "Option", value: null }, { kind: "StateCursor", value: { kind: "Int", value: null } }] };
  manifest.entrypoints[0].return_type = "Option<StateCursor<int>>";
  manifest.entrypoints[0].return_schema = schema;
  const unavailable = { noritoEncodeInstruction() { throw new Error("Native binding required"); }, noritoDecodeInstruction() { throw new Error("Native binding required"); } };
  const api = _createNoritoInstructionApi(createNativeRuntime(unavailable));
  const encoded = api.noritoEncodeInstruction({ RegisterSmartContractCode: { manifest } }, 753);
  const decoded = api.noritoDecodeInstruction(encoded, 753);
  assert.deepEqual(decoded.RegisterSmartContractCode.manifest.entrypoints[0].return_schema, schema);
  assert.deepEqual(api.noritoEncodeInstruction(decoded, 753), encoded);
});


test("durable StatePage names enforce their exact fields and matching keys", () => {
  for (const typeName of [
    "StatePage{anything: int}",
    "StatePage{items: List<(int, bool), 8>, next: Option<StateCursor<bool>>}",
    "StatePage{items: List<(Json, bool), 8>, next: Option<StateCursor<Json>>}",
  ]) assert.equal(isCanonicalKotodamaStateTypeName(typeName), false);
});


test("public Unit returns require an exact descriptor on JSON and Norito boundaries", async () => {
  const fixture = JSON.parse(await readFile(new URL("./fixtures/contract_manifest_v1.json", import.meta.url), "utf8"));
  const manifest = structuredClone(fixture.manifest);
  manifest.entrypoints[0].return_type = "()";
  manifest.entrypoints[0].return_schema = { nodes: [{ kind: "Unit", value: null }] };
  const unavailable = { noritoEncodeInstruction() { throw new Error("Native binding required"); }, noritoDecodeInstruction() { throw new Error("Native binding required"); } };
  const api = _createNoritoInstructionApi(createNativeRuntime(unavailable));
  const encoded = api.noritoEncodeInstruction({ RegisterSmartContractCode: { manifest } }, 753);
  const decoded = api.noritoDecodeInstruction(encoded, 753).RegisterSmartContractCode.manifest;
  assert.deepEqual(decoded.entrypoints[0].return_schema, manifest.entrypoints[0].return_schema);
  buildRegisterSmartContractCodeInstruction({ manifest });
  const fetchManifest = async (value) => new ToriiClient("http://localhost:8080", {
    fetchImpl: async () => new Response(JSON.stringify({ manifest: value, code_hash: null, abi_hash: null }), {
      status: 200, headers: { "content-type": "application/json" },
    }),
  }).getContractManifest("11".repeat(32));
  await fetchManifest(manifest);
  for (const fields of [["return_type"], ["return_schema"], ["return_type", "return_schema"]]) {
    for (const omitted of [false, true]) {
      const invalid = structuredClone(manifest);
      for (const field of fields) {
        if (omitted) delete invalid.entrypoints[0][field];
        else invalid.entrypoints[0][field] = null;
      }
      assert.throws(() => api.noritoEncodeInstruction({ RegisterSmartContractCode: { manifest: invalid } }, 753), /return_type.*return_schema/u);
      assert.throws(() => buildRegisterSmartContractCodeInstruction({ manifest: invalid }), /return_type.*return_schema/u);
      await assert.rejects(fetchManifest(invalid), /return_type.*return_schema/u);
    }
  }
});


test("exported structs retain locked identity in public and durable schemas", async () => {
  const fixture = JSON.parse(await readFile(new URL("../../../fixtures/kotodama/exported_structs_v1.json", import.meta.url), "utf8"));
  const vectors = JSON.parse(await readFile(new URL("../../../fixtures/kotodama/exported_struct_names_v1.json", import.meta.url), "utf8"));
  const original = "std/math@1.0.0::Math::Receipt";
  for (const name of vectors.valid) {
    const manifest = JSON.parse(JSON.stringify(fixture.manifest).replaceAll(original, name));
    validateManifestErrorTypeBindingsV1(manifest);
    const entrypoint = manifest.entrypoints[0];
    assert.equal(analyzeEntrypointValueTypeV1(entrypoint.return_schema).canonicalName, `struct ${name}`);
    assert.equal(analyzeEntrypointValueTypeV1(entrypoint.argument_schema.fields[0].ty).wordCount, 3);
    assert.equal(isCanonicalKotodamaStructName(name), true, name);
    assert.equal(isCanonicalKotodamaStateTypeName(manifest.states[0].type_name), true, name);
  }
  for (const name of vectors.invalid) {
    const manifest = JSON.parse(JSON.stringify(fixture.manifest).replaceAll(original, name));
    assert.equal(isCanonicalKotodamaStructName(name), false, name);
    assert.throws(() => analyzeEntrypointValueTypeV1(manifest.entrypoints[0].return_schema), TypeError, name);
    assert.throws(() => validateManifestErrorTypeBindingsV1(manifest), TypeError, name);
  }
  const base = JSON.parse(await readFile(new URL("./fixtures/contract_manifest_v1.json", import.meta.url), "utf8"));
  const manifest = { ...base.manifest, ...fixture.manifest };
  const unavailable = { noritoEncodeInstruction() { throw new Error("Native binding required"); }, noritoDecodeInstruction() { throw new Error("Native binding required"); } };
  const api = _createNoritoInstructionApi(createNativeRuntime(unavailable));
  const encoded = api.noritoEncodeInstruction({ RegisterSmartContractCode: { manifest } }, 753);
  const decoded = api.noritoDecodeInstruction(encoded, 753);
  assert.deepEqual(decoded.RegisterSmartContractCode.manifest.entrypoints[0].return_schema, fixture.manifest.entrypoints[0].return_schema);
  assert.deepEqual(decoded.RegisterSmartContractCode.manifest.states, fixture.manifest.states);
  assert.deepEqual(api.noritoEncodeInstruction(decoded, 753), encoded);
  manifest.error_types = [];
  assert.throws(() => validateManifestErrorTypeBindingsV1(manifest), /error_types catalog/u);
});
