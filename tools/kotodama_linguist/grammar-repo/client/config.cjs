"use strict";
const path = require("node:path");

/** Resolve one explicit workspace graph without inferring imports from open files. */
function serverConfiguration(folder, settings, fileExists) {
  const command = settings.serverPath || "koto";
  const args = ["lsp"];
  let project;
  if (folder) {
    const configured = (settings.project || "kotodama.project.json").replaceAll("${workspaceFolder}", folder);
    project = path.resolve(folder, configured);
    if (fileExists(project)) args.push("--project", project);
    else project = undefined;
  }
  if (settings.zk) args.push("--zk");
  return { command, args, project };
}
/** Select only the configured manifest; ordinary JSON files retain their own language providers. */
function isProjectManifest(project, document) {
  return Boolean(project && document.uri.scheme === "file" && path.resolve(document.uri.fsPath) === project);
}
/**
 * Validate a "Run test" code lens issued by `koto lsp` and return the exact process to run.
 * Only `koto test run --filter <name> --exact <source>` is accepted; arguments are passed
 * without a shell.
 */
function testRunConfiguration(serverPath, lens) {
  const args = lens && Array.isArray(lens.args) ? lens.args : [];
  const valid = args.length === 6
    && args.every(argument => typeof argument === "string" && argument.length > 0)
    && args[0] === "test" && args[1] === "run" && args[2] === "--filter"
    && args[3] === lens.name && args[4] === "--exact" && !args[5].startsWith("-");
  if (!valid) throw new Error("Kotodama: the language server sent a malformed test lens");
  return { command: serverPath || "koto", args: [...args] };
}
module.exports = { serverConfiguration, isProjectManifest, testRunConfiguration };
