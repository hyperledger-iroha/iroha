import assert from "node:assert/strict";
import test from "node:test";

import { normalizeCompilerResult } from "../src/kotodamaCompiler/normalize.js";
import { makeNativeTest, nativeBinding } from "./helpers/native.js";

const nativeTest = makeNativeTest(test, { require: "compileKotodama" });
const RESULT_FIELDS = ["diagnosticsJson", "ok", "output"];

nativeTest("native Kotodama success uses an explicit null diagnostics sentinel", async () => {
  const raw = await nativeBinding.compileKotodama({
    source: "seiyaku Demo { view fn ping() -> int { return 1; } }",
    zk: false,
  });

  assert.deepEqual(Object.keys(raw).sort(), RESULT_FIELDS);
  assert.equal(raw.ok, true);
  assert.notEqual(raw.output, null);
  assert.equal(raw.diagnosticsJson, null);
  assert.equal(normalizeCompilerResult(raw).ok, true);
});

nativeTest("native Kotodama failure uses an explicit null output sentinel", async () => {
  const raw = await nativeBinding.compileKotodama({
    source: "seiyaku Demo {\n🙂\n}",
    zk: false,
  });

  assert.deepEqual(Object.keys(raw).sort(), RESULT_FIELDS);
  assert.equal(raw.ok, false);
  assert.equal(raw.output, null);
  assert.equal(typeof raw.diagnosticsJson, "string");
  const normalized = normalizeCompilerResult(raw);
  assert.equal(normalized.ok, false);
  assert.ok(normalized.diagnostics.length > 0);
});


nativeTest("native Kotodama V1 preserves declared arguments and composable value schemas", async () => {
  const source = `seiyaku NativeValues {
    const int PAGE_SIZE = 2;
    error enum VaultError { CapacityExceeded = 7; ZeroDeposit = 1; }
    state () marker;
    state StateMap<int, bool> Flags;

    hajimari() { marker = (); }
    kaizen() {}

    fn combine(int _ value, int minimum, int maximum) -> int {
      value + minimum + maximum
    }

    kotoage fn deposit(() _ receipt, int amount) -> Result<(), VaultError> authorize("Deposit") {
      marker = receipt;
      if amount == 0 { return Result::err(VaultError::ZeroDeposit); }
      let adjusted = combine(amount, maximum: 10, minimum: 0);
      Flags[adjusted] = true;
      Result::ok(())
    }

    view fn read_marker() -> () { marker }
    view fn nested(Option<Option<()>> value) -> Option<Option<()>> { value }
    view fn resume(Option<StateCursor<int>> after) -> Option<StateCursor<int>> { after }
    view fn browse(Option<StateCursor<int>> after) -> StatePage<int, bool, PAGE_SIZE * 2> {
      Flags.page(limit: PAGE_SIZE * 2, after: after)
    }
  }`;
  const request = { source, sourceName: "contracts/native-values.ko", zk: false };
  const raw = await nativeBinding.compileKotodama(request);
  assert.deepEqual(Object.keys(raw).sort(), RESULT_FIELDS);
  assert.equal(raw.ok, true, raw.diagnosticsJson);
  assert.equal(raw.diagnosticsJson, null);
  const result = normalizeCompilerResult(raw);
  assert.equal(result.ok, true);
  const { manifest } = result.output;
  assert.equal(manifest.seiyaku_name, "NativeValues");

  const entries = new Map(manifest.entrypoints.map((entry) => [entry.name, entry]));
  assert.deepEqual([...entries.keys()].sort(), [
    "browse", "deposit", "hajimari", "kaizen", "nested", "read_marker", "resume",
  ]);
  assert.equal(manifest.entrypoints.length, entries.size);
  const unit = { kind: "Unit", value: null };
  const option = { kind: "Option", value: null };
  const integer = { kind: "Leaf", value: { kind: "Int", value: null } };
  const cursor = { kind: "StateCursor", value: { kind: "Int", value: null } };
  const error = {
    identity: "NativeValues::VaultError",
    variants: [
      { name: "ZeroDeposit", code: 1 },
      { name: "CapacityExceeded", code: 7 },
    ],
  };
  assert.deepEqual(
    manifest.error_types.filter((descriptor) => descriptor.identity === error.identity),
    [error],
  );
  assert.deepEqual(manifest.states, [
    { name: "marker", type_name: "()" },
    { name: "Flags", type_name: "StateMap<int, bool>" },
  ]);

  // Source labels do not rename fields in the public JSON argument record.
  const deposit = entries.get("deposit");
  assert.deepEqual(deposit.kind, { kind: "Kotoage", value: null });
  assert.equal(deposit.permission, "Deposit");
  assert.deepEqual(deposit.params, [
    { name: "receipt", type_name: "()" },
    { name: "amount", type_name: "int" },
  ]);
  assert.deepEqual(deposit.argument_schema, {
    fields: [
      { name: "receipt", ty: { nodes: [unit] } },
      { name: "amount", ty: { nodes: [integer] } },
    ],
  });
  assert.equal(deposit.return_type, "Result<(), NativeValues::VaultError>");
  assert.deepEqual(deposit.return_schema, {
    nodes: [{ kind: "Result", value: null }, unit, { kind: "Error", value: error }],
  });

  // Unit's schema is present with canonical null variant metadata. This test
  // compiles a contract; it does not execute a Unit return or a state scan.
  for (const [name, kind] of [
    ["hajimari", "Hajimari"], ["kaizen", "Kaizen"], ["read_marker", "View"],
  ]) {
    const entry = entries.get(name);
    assert.deepEqual(entry.kind, { kind, value: null });
    assert.deepEqual(entry.params, []);
    assert.equal(entry.argument_schema, null);
    assert.equal(entry.return_type, "()");
    assert.deepEqual(entry.return_schema, { nodes: [unit] });
  }
  const nested = entries.get("nested");
  assert.deepEqual(nested.params, [{ name: "value", type_name: "Option<Option<()>>" }]);
  assert.deepEqual(nested.argument_schema, {
    fields: [{ name: "value", ty: { nodes: [option, option, unit] } }],
  });
  assert.equal(nested.return_type, "Option<Option<()>>");
  assert.deepEqual(nested.return_schema, { nodes: [option, option, unit] });

  for (const name of ["resume", "browse"]) {
    const entry = entries.get(name);
    assert.deepEqual(entry.kind, { kind: "View", value: null });
    assert.deepEqual(entry.params, [{ name: "after", type_name: "Option<StateCursor<int>>" }]);
    assert.deepEqual(entry.argument_schema, {
      fields: [{ name: "after", ty: { nodes: [option, cursor] } }],
    });
  }
  assert.equal(entries.get("resume").return_type, "Option<StateCursor<int>>");
  assert.deepEqual(entries.get("resume").return_schema, { nodes: [option, cursor] });
  assert.equal(entries.get("browse").return_type, "StatePage<int, bool, 4>");
  assert.deepEqual(entries.get("browse").return_schema, {
    nodes: [
      { kind: "Struct", value: { name: "StatePage", fields: ["items", "next"] } },
      { kind: "List", value: { capacity: 4 } },
      { kind: "Tuple", value: 2 },
      integer,
      { kind: "Leaf", value: { kind: "Bool", value: null } },
      option,
      cursor,
    ],
  });

  // Call modes belong to compiler source interfaces, not public record fields.
  // Keep the call unchanged and alter only one declaration marker at a time.
  const signature = "fn combine(int _ value, int minimum, int maximum)";
  assert.equal(source.split(signature).length, 2);
  for (const [replacement, expectedCode] of [
    ["fn combine(int value, int minimum, int maximum)", "E_NAMED_ARGUMENTS_REQUIRED"],
    ["fn combine(int _ value, int _ minimum, int maximum)", "E_POSITIONAL_ARGUMENT_REQUIRED"],
  ]) {
    const rejectedRaw = await nativeBinding.compileKotodama({
      ...request, source: source.replace(signature, replacement),
    });
    assert.equal(rejectedRaw.ok, false, replacement);
    assert.equal(rejectedRaw.output, null);
    const rejected = normalizeCompilerResult(rejectedRaw);
    assert.equal(rejected.ok, false);
    assert.deepEqual(
      rejected.diagnostics.filter((diagnostic) => diagnostic.severity === "error")
        .map((diagnostic) => diagnostic.code),
      [expectedCode],
      replacement,
    );
  }
});
