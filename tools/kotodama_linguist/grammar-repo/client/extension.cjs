"use strict";
const vscode = require("vscode");
const fs = require("node:fs");
const path = require("node:path");
const { LanguageClient } = require("vscode-languageclient/node");
const { serverConfiguration, isProjectManifest } = require("./config.cjs");
const clients = new Map();
let restartPromise = Promise.resolve();

async function startFolder(context, folder) {
  const key = folder ? folder.uri.toString() : "untitled";
  if (clients.has(key)) return;
  const settings = vscode.workspace.getConfiguration("kotodama", folder?.uri);
  const config = serverConfiguration(folder?.uri.fsPath, {
    serverPath: settings.get("serverPath"), project: settings.get("project"), zk: settings.get("zk"),
  }, fs.existsSync);
  const selector = folder
    ? [{ language: "kotodama", scheme: "file", pattern: new vscode.RelativePattern(folder, "**/*.ko") }]
    : [{ language: "kotodama", scheme: "untitled" }];
  const watchers = folder ? [
    vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(folder, "**/*.ko")),
    vscode.workspace.createFileSystemWatcher(config.project
      ? new vscode.RelativePattern(path.dirname(config.project), path.basename(config.project))
      : new vscode.RelativePattern(folder, "**/kotodama.project.json")),
  ] : [];
  const client = new LanguageClient(`kotodama:${key}`, "Kotodama", {
    command: config.command, args: config.args, options: { cwd: folder?.uri.fsPath },
  }, {
    documentSelector: selector, workspaceFolder: folder,
    synchronize: { fileEvents: watchers },
  });
  clients.set(key, { client, watchers });
  try {
    await client.start();
    // Synchronize the exact metadata buffer without advertising Kotodama formatting,
    // completion, or hover for JSON documents. Rename versions cover this buffer too.
    const matches = document => isProjectManifest(config.project, document);
    const opened = new Set();
    let notifications = Promise.resolve();
    const send = (method, params) => {
      notifications = notifications.then(() => client.sendNotification(method, params))
        .catch(error => client.error("Failed to synchronize the Kotodama project manifest", error));
    };
    const open = document => {
      if (!matches(document) || opened.has(document.uri.toString())) return;
      opened.add(document.uri.toString());
      send("textDocument/didOpen", { textDocument: { uri: document.uri.toString(), languageId: "json", version: document.version, text: document.getText() } });
    };
    watchers.push(vscode.workspace.onDidOpenTextDocument(open));
    watchers.push(vscode.workspace.onDidChangeTextDocument(event => {
      if (!matches(event.document)) return;
      if (!opened.has(event.document.uri.toString())) { open(event.document); return; }
      send("textDocument/didChange", { textDocument: { uri: event.document.uri.toString(), version: event.document.version }, contentChanges: [{ text: event.document.getText() }] });
    }));
    watchers.push(vscode.workspace.onDidCloseTextDocument(document => {
      if (!opened.delete(document.uri.toString())) return;
      send("textDocument/didClose", { textDocument: { uri: document.uri.toString() } });
    }));
    for (const document of vscode.workspace.textDocuments) open(document);
  } catch (error) {
    clients.delete(key);
    watchers.forEach(watcher => watcher.dispose());
    await vscode.window.showErrorMessage(`Kotodama could not start ${config.command}: ${error.message}. Install the koto toolchain or set Kotodama: Server Path.`);
  }
}

async function stopAll() {
  const previous = [...clients.values()];
  clients.clear();
  await Promise.allSettled(previous.map(async ({ client, watchers }) => {
    watchers.forEach(watcher => watcher.dispose());
    await client.stop();
  }));
}

async function activate(context) {
  const start = async () => {
    for (const folder of vscode.workspace.workspaceFolders || []) await startFolder(context, folder);
    await startFolder(context, undefined);
  };
  const restart = () => {
    restartPromise = restartPromise.then(async () => { await stopAll(); await start(); });
    return restartPromise;
  };
  context.subscriptions.push(vscode.commands.registerCommand("kotodama.restart", restart));
  context.subscriptions.push(vscode.workspace.onDidChangeConfiguration(event => {
    if (event.affectsConfiguration("kotodama")) void restart();
  }));
  context.subscriptions.push(vscode.workspace.onDidChangeWorkspaceFolders(() => { void restart(); }));
  await start();
}
async function deactivate() { await restartPromise; await stopAll(); }
module.exports = { activate, deactivate };
