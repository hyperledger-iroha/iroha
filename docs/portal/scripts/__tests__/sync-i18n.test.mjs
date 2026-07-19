import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildLocalePath,
  buildStub,
  isTranslationFile,
  parseRefreshPrefixes,
  shouldRefreshTranslation,
} from '../sync-i18n.mjs';

const locale = {
  code: 'es',
  name: 'Spanish',
  direction: 'ltr',
  heading: '# Traduccion en curso',
  body: ['Cuerpo de marcador.'],
  wrapRtl: false,
};

test('buildLocalePath writes under Docusaurus current locale tree', () => {
  const target = buildLocalePath('nexus/overview.md', 'es');

  assert.match(
    target,
    /i18n[/\\]es[/\\]docusaurus-plugin-content-docs[/\\]current[/\\]nexus[/\\]overview\.md$/,
  );
});

test('buildStub includes source traceability metadata', () => {
  const stub = buildStub(
    'nexus/overview.md',
    locale,
    {id: 'nexus-overview', slug: '/nexus'},
    {
      hash: 'a'.repeat(64),
      modified: '2026-06-22T00:00:00.000Z',
    },
  );

  assert.match(stub, /^id: nexus-overview$/m);
  assert.match(stub, /^slug: \/nexus$/m);
  assert.match(stub, /^source: docs\/portal\/docs\/nexus\/overview\.md$/m);
  assert.match(stub, /^source_hash: a{64}$/m);
  assert.match(stub, /^source_last_modified: "2026-06-22T00:00:00.000Z"$/m);
  assert.match(stub, /^translation_last_reviewed: null$/m);
});

test('buildStub rejects missing source metadata', () => {
  assert.throws(
    () => buildStub('nexus/overview.md', locale),
    /source metadata is required/,
  );
});

test('refresh prefixes are relative and scope matching to a docs subtree', () => {
  const prefixes = parseRefreshPrefixes(['--refresh-prefix=./sns/']);

  assert.deepEqual(prefixes, ['sns']);
  assert.equal(shouldRefreshTranslation('sns/registrar-api.md', prefixes), true);
  assert.equal(shouldRefreshTranslation('sns.md', prefixes), false);
  assert.equal(shouldRefreshTranslation('nexus/sns.md', prefixes), false);
});

test('refresh prefixes reject traversal and unknown arguments', () => {
  assert.throws(
    () => parseRefreshPrefixes(['--refresh-prefix=../sns']),
    /invalid refresh prefix/,
  );
  assert.throws(
    () => parseRefreshPrefixes(['--refresh-prefix=sns/../nexus']),
    /invalid refresh prefix/,
  );
  assert.throws(() => parseRefreshPrefixes(['--refresh-all']), /unknown argument/);
});

test('localized source filenames are distinguished from canonical docs', () => {
  assert.equal(isTranslationFile('registrar-api.fr.md'), true);
  assert.equal(isTranslationFile('registrar-api.zh-hans.mdx'), true);
  assert.equal(isTranslationFile('registrar-api.md'), false);
});
