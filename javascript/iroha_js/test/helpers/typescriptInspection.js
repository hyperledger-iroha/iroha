// TypeScript 7 inspection projects use virtual configs and never execute SDK code.
import assert from "node:assert/strict";
import path from "node:path";
import { after } from "node:test";
import { API } from "typescript/unstable/sync";

const files = new Map();
const snapshots = [];
const api = new API({ fs: { readFile: (name) => files.get(name) } });
let nextProject = 0;

after(() => {
  for (const snapshot of snapshots) snapshot.dispose();
  api.close();
});

/** Open an exact declaration/consumer project with TypeScript's current checker. */
export function inspectTypeScriptProject(fileName, options = {}, sourceText) {
  const original = path.resolve(fileName);
  const config = path.join(path.dirname(original), `.iroha-inspection-${nextProject++}.json`);
  if (sourceText !== undefined) files.set(original, sourceText);
  files.set(config, JSON.stringify({
    compilerOptions: {
      strict: true, target: "es2022", module: "nodenext", moduleResolution: "nodenext",
      skipLibCheck: false, noEmit: true, removeComments: true, ...options,
    },
    files: [original],
  }));
  const snapshot = api.updateSnapshot({ openProjects: [config], fileChanges: { invalidateAll: true } });
  snapshots.push(snapshot);
  const project = snapshot.getProject(config);
  assert.ok(project, "TypeScript inspection project must load");
  const source = project.program.getSourceFile(original);
  assert.ok(source, `TypeScript source must load: ${original}`);
  return project;
}

/** Parse captured text without loading its imports or allowing syntax recovery. */
export function parseTypeScriptSource(fileName, sourceText) {
  const project = inspectTypeScriptProject(fileName, {
    allowJs: true, noLib: true, noResolve: true,
  }, sourceText);
  assert.deepEqual(project.program.getSyntacticDiagnostics(), [], "source must parse without recovery");
  return project.program.getSourceFile(path.resolve(fileName));
}

