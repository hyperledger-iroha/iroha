import assert from 'node:assert/strict';
import test from 'node:test';

import {
  MANIFEST_VERSION,
  TEMPLATE_REVISION,
  formatLedgerSection,
  formatSdkGuideSection,
  manifestNeedsUpdate
} from '../sync-norito-snippets.mjs';

function entry(overrides = {}) {
  return {
    slug: overrides.slug ?? 'demo',
    source: overrides.source ?? 'crates/demo.ko',
    title: overrides.title ?? 'Demo',
    description: overrides.description ?? 'Example snippet',
    size: overrides.size ?? 64,
    mtimeMs: overrides.mtimeMs ?? 1_700_000_000_000,
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

test('manifestNeedsUpdate triggers when template revision changes', () => {
  const downgradedRevision = TEMPLATE_REVISION === 0 ? -1 : TEMPLATE_REVISION - 1;
  const previous = {
    version: MANIFEST_VERSION,
    entries: [entry({templateRevision: downgradedRevision})]
  };
  const nextEntries = [entry()];
  assert.strictEqual(manifestNeedsUpdate(previous, nextEntries), true);
});
