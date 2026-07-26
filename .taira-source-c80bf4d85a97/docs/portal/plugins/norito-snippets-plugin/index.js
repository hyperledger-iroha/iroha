import {execFile} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {promisify} from 'node:util';

import {SNIPPETS} from '../../scripts/norito-snippets-config.mjs';

const execFileAsync = promisify(execFile);

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const portalRoot = path.resolve(__dirname, '..', '..');
const repoRoot = path.resolve(portalRoot, '..', '..');
const syncScript = path.resolve(portalRoot, 'scripts', 'sync-norito-snippets.mjs');
const configPath = path.resolve(portalRoot, 'scripts', 'norito-snippets-config.mjs');

let inFlightSync = null;

export function collectSnippetPaths({repoRoot: root = repoRoot, snippets = SNIPPETS} = {}) {
  const seen = new Set();
  const resolved = [];
  for (const snippet of snippets ?? []) {
    if (!snippet || typeof snippet.source !== 'string' || snippet.source.trim() === '') {
      continue;
    }
    const absolutePath = path.resolve(root, snippet.source);
    if (seen.has(absolutePath)) {
      continue;
    }
    seen.add(absolutePath);
    resolved.push(absolutePath);
  }
  return resolved;
}

async function runSyncScript() {
  if (!inFlightSync) {
    inFlightSync = execFileAsync('node', [syncScript], {
      cwd: portalRoot,
      stdio: 'inherit',
    }).finally(() => {
      inFlightSync = null;
    });
  }
  return inFlightSync;
}

export default function noritoSnippetsPlugin() {
  return {
    name: 'norito-snippets-plugin',
    getPathsToWatch() {
      return [...collectSnippetPaths(), syncScript, configPath];
    },
    async loadContent() {
      await runSyncScript();
    },
  };
}
