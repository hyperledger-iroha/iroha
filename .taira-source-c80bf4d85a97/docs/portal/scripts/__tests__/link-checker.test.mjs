import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdtemp, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import test from 'node:test';

import {
  loadManifestMetadata,
  parseOptions,
  parseSitemap,
  relativePathsFor,
  urlToPathname,
} from '../check-links.mjs';

test('parseOptions keeps defaults', () => {
  const options = parseOptions([]);
  assert.ok(options.buildDir.endsWith('docs/portal/build') || options.buildDir.endsWith('build'));
});

test('parseOptions accepts build override', () => {
  const options = parseOptions(['--build-dir=./tmp/build']);
  assert.ok(options.buildDir.endsWith('tmp/build'));
});

test('parseSitemap extracts locations', () => {
  const xml = '<urlset><url><loc>https://example/a</loc></url><url><loc>https://example/b</loc></url></urlset>';
  assert.deepEqual(parseSitemap(xml), ['https://example/a', 'https://example/b']);
});

test('urlToPathname parses URL paths', () => {
  assert.equal(urlToPathname('https://example/a/b'), '/a/b');
  assert.equal(urlToPathname('not-a-url'), '');
});

test('relativePathsFor generates candidates', () => {
  const candidates = relativePathsFor('/reference');
  assert.ok(candidates.includes('reference.html'));
  assert.ok(candidates.includes('reference/index.html'));
  assert.ok(candidates.includes('reference'));
});

test('relativePathsFor trims trailing slashes', () => {
  const candidates = relativePathsFor('/sdks/');
  assert.ok(candidates.includes('sdks.html'));
  assert.ok(candidates.includes('sdks/index.html'));
});

test('loadManifestMetadata returns null when manifest absent', async () => {
  const tmp = await mkdtemp(join(tmpdir(), 'link-checker-'));
  const metadata = await loadManifestMetadata(tmp);
  assert.equal(metadata, null);
});

test('loadManifestMetadata captures manifest stats', async () => {
  const tmp = await mkdtemp(join(tmpdir(), 'link-checker-'));
  const manifestPath = join(tmp, 'checksums.sha256');
  const contents = 'abc123  index.html\nffffee  assets/main.js\n';
  await writeFile(manifestPath, contents, 'utf8');

  const metadata = await loadManifestMetadata(tmp);
  assert.ok(metadata);
  assert.equal(metadata.path, 'checksums.sha256');
  assert.equal(metadata.entries, 2);
  assert.equal(metadata.bytes, Buffer.byteLength(contents));
  const expectedHash = createHash('sha256').update(contents).digest('hex');
  assert.equal(metadata.id, expectedHash);
});
