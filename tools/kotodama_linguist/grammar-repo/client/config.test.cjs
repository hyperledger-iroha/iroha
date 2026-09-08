"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const path = require("node:path");
const { serverConfiguration, isProjectManifest } = require("./config.cjs");

test("uses the configured executable and exact workspace graph as separate arguments", () => {
  const folder = path.resolve("space 日本");
  const project = path.join(folder, "kotodama.project.json");
  const result = serverConfiguration(folder, { serverPath: "koto path", project: "${workspaceFolder}/kotodama.project.json", zk: true }, value => value === project);
  assert.deepEqual(result, { command: "koto path", args: ["lsp", "--project", project, "--zk"], project });
});
test("missing graphs and untitled documents never invent graph authority", () => {
  assert.deepEqual(serverConfiguration(path.resolve("workspace"), {}, () => false), { command: "koto", args: ["lsp"], project: undefined });
  assert.deepEqual(serverConfiguration(undefined, {}, () => { throw Error("unexpected filesystem lookup"); }), { command: "koto", args: ["lsp"], project: undefined });
});
test("only the exact configured manifest receives metadata buffer synchronization", () => {
  const project = path.resolve("金庫😀/custom-project.json");
  const document = { uri: { scheme: "file", fsPath: project } };
  assert.equal(isProjectManifest(project, document), true);
  assert.equal(isProjectManifest(undefined, document), false);
  assert.equal(isProjectManifest(project, { uri: { scheme: "untitled", fsPath: project } }), false);
  assert.equal(isProjectManifest(project, { uri: { scheme: "file", fsPath: path.resolve("other/custom-project.json") } }), false);
});
