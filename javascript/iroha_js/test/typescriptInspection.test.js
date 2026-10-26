import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";

import { inspectTypeScriptProject, parseTypeScriptSource } from "./helpers/typescriptInspection.js";

test("TypeScript inspection checks captured source without executing its imports", () => {
  const fileName = path.resolve("captured-inspection.ts");
  const source = 'import "./must-not-be-loaded.js";\nexport const amount: number = "wrong";';
  const { program } = inspectTypeScriptProject(fileName, { noResolve: true }, source);
  assert.equal(program.getSourceFile(fileName).text, source);
  assert.ok(program.getSemanticDiagnostics().some((diagnostic) => diagnostic.code === 2322));
});

test("TypeScript parsing rejects malformed source rather than accepting recovery", () => {
  assert.throws(() => parseTypeScriptSource("invalid-inspection.js", "const value = ;"),
    /source must parse without recovery/u);
});

test("TypeScript snapshots preserve earlier captured source after a replacement", () => {
  const first = parseTypeScriptSource("snapshot-inspection.js", 'const label = "誓約";');
  const second = parseTypeScriptSource("snapshot-inspection.js", 'const label = "changed";');
  assert.equal(first.text, 'const label = "誓約";');
  assert.equal(second.text, 'const label = "changed";');
});
