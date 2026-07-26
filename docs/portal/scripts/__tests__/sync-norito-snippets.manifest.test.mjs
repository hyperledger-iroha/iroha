import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rm,
  writeFile
} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {fileURLToPath} from 'node:url';

import {SNIPPETS} from '../norito-snippets-config.mjs';
import {
  MANIFEST_VERSION,
  TEMPLATE_REVISION,
  containsSeiyakuDeclaration,
  formatLedgerSection,
  formatSdkGuideSection,
  manifestNeedsUpdate,
  parseCliMode
} from '../sync-norito-snippets.mjs';

const testDirectory = path.dirname(fileURLToPath(import.meta.url));
const portalRoot = path.resolve(testDirectory, '..', '..');
const repositoryRoot = path.resolve(portalRoot, '..', '..');

test('CLI mode is fail-closed and has an explicit non-mutating check', () => {
  assert.strictEqual(parseCliMode([]), 'write');
  assert.strictEqual(parseCliMode(['--check']), 'check');
  assert.throws(() => parseCliMode(['--write']));
  assert.throws(() => parseCliMode(['--check', '--write']));
  assert.throws(() => parseCliMode(['--check', 'unexpected']));
});

test('quickstart matching accepts only canonical seiyaku spellings', () => {
  assert.strictEqual(containsSeiyakuDeclaration('seiyaku Hello {}', 'Hello'), true);
  assert.strictEqual(containsSeiyakuDeclaration('誓約 Hello {}', 'Hello'), true);
  assert.strictEqual(containsSeiyakuDeclaration('contract Hello {}', 'Hello'), false);
});

function entry(overrides = {}) {
  return {
    slug: overrides.slug ?? 'demo',
    source: overrides.source ?? 'crates/demo.ko',
    title: overrides.title ?? 'Demo',
    description: overrides.description ?? 'Example snippet',
    renderConfigDigest:
      overrides.renderConfigDigest ??
      JSON.stringify({
        ledgerWalkthrough: ['Compile contract'],
        sdkGuides: [{label: 'Rust SDK quickstart', permalink: '/sdks/rust'}]
      }),
    size: overrides.size ?? 64,
    sourceSha256:
      overrides.sourceSha256 ??
      '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
    templateRevision: overrides.templateRevision ?? TEMPLATE_REVISION
  };
}

test('manifestNeedsUpdate returns true when manifest missing', () => {
  assert.strictEqual(manifestNeedsUpdate(null, [entry()]), true);
});

test('manifestNeedsUpdate detects version mismatch', () => {
  const previous = {version: MANIFEST_VERSION - 1, entries: [entry()]};
  assert.strictEqual(manifestNeedsUpdate(previous, [entry()]), true);
});

test('formatLedgerSection renders bullet list', () => {
  const section = formatLedgerSection(['Compile contract', 'Deploy contract']);
  assert.match(section, /## Ledger walkthrough/);
  assert.match(section, /- Compile contract/);
  assert.match(section, /- Deploy contract/);
});

test('formatSdkGuideSection skips invalid entries', () => {
  const section = formatSdkGuideSection([
    {label: 'Rust SDK quickstart', permalink: '/sdks/rust'},
    {label: '', permalink: '/invalid'},
    {label: 'Missing link'}
  ]);
  assert.match(section, /## Related SDK guides/);
  assert.match(section, /\[Rust SDK quickstart\]\(\/sdks\/rust\)/);
  assert.doesNotMatch(section, /invalid/);
});

test('manifestNeedsUpdate ignores entry order', () => {
  const previous = {
    version: MANIFEST_VERSION,
    entries: [entry({slug: 'b'}), entry({slug: 'a'})]
  };
  const nextEntries = [entry({slug: 'a'}), entry({slug: 'b'})];
  assert.strictEqual(manifestNeedsUpdate(previous, nextEntries), false);
});

test('manifestNeedsUpdate detects metadata changes', () => {
  const previous = {version: MANIFEST_VERSION, entries: [entry()]};
  const nextEntries = [entry({description: 'Updated'})];
  assert.strictEqual(manifestNeedsUpdate(previous, nextEntries), true);
});

test('manifestNeedsUpdate detects source content changes independent of mtime', () => {
  const previous = {version: MANIFEST_VERSION, entries: [entry()]};
  const nextEntries = [
    entry({
      sourceSha256:
        'abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789'
    })
  ];
  assert.strictEqual(manifestNeedsUpdate(previous, nextEntries), true);
});

test('manifestNeedsUpdate detects walkthrough changes', () => {
  const previous = {version: MANIFEST_VERSION, entries: [entry()]};
  const nextEntries = [
    entry({
      renderConfigDigest: JSON.stringify({
        ledgerWalkthrough: ['Compile contract', 'Call deposit(amount)'],
        sdkGuides: [{label: 'Rust SDK quickstart', permalink: '/sdks/rust'}]
      })
    })
  ];
  assert.strictEqual(manifestNeedsUpdate(previous, nextEntries), true);
});

test('CLI write/write/check is deterministic without the ignored cache', async (context) => {
  const temporaryRoot = await realpath(
    await mkdtemp(path.join(tmpdir(), 'norito-snippets-cli-'))
  );
  context.after(() => rm(temporaryRoot, {recursive: true, force: true}));

  const copy = async (relative) => {
    const source = path.join(repositoryRoot, relative);
    const destination = path.join(temporaryRoot, relative);
    await mkdir(path.dirname(destination), {recursive: true});
    await copyFile(source, destination);
  };

  await copy('docs/portal/scripts/sync-norito-snippets.mjs');
  await copy('docs/portal/scripts/norito-snippets-config.mjs');
  await copy('docs/portal/docs/norito/quickstart.md');
  await copy('docs/portal/docs/norito/examples/index.md');
  for (const snippet of SNIPPETS) {
    await copy(snippet.source);
    await copy(`docs/portal/docs/norito/examples/${snippet.slug}.md`);
    await copy(`docs/portal/static/norito-snippets/${snippet.slug}.ko`);
  }

  const script = path.join(
    temporaryRoot,
    'docs/portal/scripts/sync-norito-snippets.mjs'
  );
  const run = (...arguments_) =>
    spawnSync(process.execPath, [script, ...arguments_], {
      cwd: path.join(temporaryRoot, 'docs/portal'),
      encoding: 'utf8'
    });
  const manifest = path.join(
    temporaryRoot,
    'docs/portal/.docusaurus/norito-snippets-manifest.json'
  );

  const first = run();
  assert.strictEqual(first.status, 0, first.stderr);
  const firstManifest = await readFile(manifest, 'utf8');
  const parsed = JSON.parse(firstManifest);
  assert.strictEqual(parsed.version, MANIFEST_VERSION);
  assert.strictEqual(Object.hasOwn(parsed, 'generatedAt'), false);
  assert.strictEqual(parsed.entries.length, SNIPPETS.length);
  for (const manifestEntry of parsed.entries) {
    assert.match(manifestEntry.sourceSha256, /^[0-9a-f]{64}$/u);
    assert.strictEqual(Object.hasOwn(manifestEntry, 'mtimeMs'), false);
  }

  const second = run();
  assert.strictEqual(second.status, 0, second.stderr);
  assert.strictEqual(await readFile(manifest, 'utf8'), firstManifest);

  await rm(manifest);
  const cleanCloneCheck = run('--check');
  assert.strictEqual(cleanCloneCheck.status, 0, cleanCloneCheck.stderr);
  await assert.rejects(readFile(manifest, 'utf8'), {code: 'ENOENT'});

  const retiredOutput = path.join(
    temporaryRoot,
    'docs/portal/static/norito-snippets/init-entrypoint.ko'
  );
  await writeFile(retiredOutput, 'retired must remain visible to the test', 'utf8');
  const retiredCheck = run('--check');
  assert.notStrictEqual(retiredCheck.status, 0);
  assert.match(retiredCheck.stderr, /retired generated snippet remains/u);
  assert.strictEqual(
    await readFile(retiredOutput, 'utf8'),
    'retired must remain visible to the test',
    'check mode must not remove a retired output'
  );
  await rm(retiredOutput);

  const corruptedOutput = path.join(
    temporaryRoot,
    'docs/portal/static/norito-snippets',
    `${SNIPPETS[0].slug}.ko`
  );
  const corrupted = `${await readFile(corruptedOutput, 'utf8')}\ncorrupt`;
  await writeFile(corruptedOutput, corrupted, 'utf8');
  const failedCheck = run('--check');
  assert.notStrictEqual(failedCheck.status, 0);
  assert.match(failedCheck.stderr, /stale generated snippet output/u);
  assert.strictEqual(
    await readFile(corruptedOutput, 'utf8'),
    corrupted,
    'check mode must not repair a stale output'
  );
});

test('manifestNeedsUpdate triggers when template revision changes', () => {
  const downgradedRevision = TEMPLATE_REVISION === 0 ? -1 : TEMPLATE_REVISION - 1;
  const previous = {
    version: MANIFEST_VERSION,
    entries: [entry({templateRevision: downgradedRevision})]
  };
  const nextEntries = [entry()];
  assert.strictEqual(manifestNeedsUpdate(previous, nextEntries), true);
});
