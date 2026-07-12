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
