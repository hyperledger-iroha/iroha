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
module.exports = { serverConfiguration, isProjectManifest };
