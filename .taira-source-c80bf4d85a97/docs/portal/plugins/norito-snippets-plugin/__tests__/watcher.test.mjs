import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';
import {fileURLToPath} from 'node:url';

import noritoSnippetsPlugin, {collectSnippetPaths} from '../index.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const portalRoot = path.resolve(__dirname, '..', '..', '..');
const repoRoot = path.resolve(portalRoot, '..', '..');
const syncScriptPath = path.resolve(portalRoot, 'scripts', 'sync-norito-snippets.mjs');
const configPath = path.resolve(portalRoot, 'scripts', 'norito-snippets-config.mjs');

test('collectSnippetPaths resolves and deduplicates snippet sources', () => {
  const repo = '/tmp/norito-snippets';
  const paths = collectSnippetPaths({
    repoRoot: repo,
    snippets: [
      {source: 'alpha.ko'},
      {source: './beta.ko'},
      {source: 'nested/gamma.ko'},
      {source: 'alpha.ko'},
      {source: ''},
      {},
    ],
  });

  assert.deepEqual(paths, [
    path.resolve(repo, 'alpha.ko'),
    path.resolve(repo, './beta.ko'),
    path.resolve(repo, 'nested/gamma.ko'),
  ]);
});

test('plugin registers snippet, script, and config watch paths', () => {
  const plugin = noritoSnippetsPlugin();
  const watchPaths = plugin.getPathsToWatch();

  assert.ok(
    watchPaths.includes(syncScriptPath),
    'watch list should include the sync script path',
  );
  assert.ok(
    watchPaths.includes(configPath),
    'watch list should include the snippet config path',
  );

  for (const snippetPath of collectSnippetPaths({repoRoot})) {
    assert.ok(
      watchPaths.includes(snippetPath),
      `watch list should include ${snippetPath}`,
    );
  }
});
