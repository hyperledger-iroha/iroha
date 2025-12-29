import test from 'node:test';
import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';
import {dirname, join} from 'node:path';
import {mkdtemp, readFile, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';

import {
  encodeBase32Lower,
  generateGatewayBinding,
  formatHeaders,
} from '../generate-gateway-binding.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const fixturePath = join(__dirname, 'fixtures', 'sample.manifest.json');

test('encodeBase32Lower matches rust implementation', () => {
  const bytes = Uint8Array.from([0xde, 0xad, 0xbe, 0xef]);
  assert.equal(encodeBase32Lower(bytes), '32w353y');
  assert.equal(encodeBase32Lower(new Uint8Array([])), '');
});

test('generateGatewayBinding emits headers + metadata', async () => {
  const now = new Date('2026-03-10T12:00:00.000Z');
  const binding = await generateGatewayBinding({
    manifestJsonPath: fixturePath,
    alias: 'docs.sora',
    hostname: 'docs.sora',
    proofStatus: 'ok',
    routeLabel: 'prod',
    now,
  });

  assert.equal(binding.alias, 'docs.sora');
  assert.equal(binding.hostname, 'docs.sora');
  assert.equal(binding.contentCid, 'bafyr6icue6y6q7px7xiujin46u2aqwuqrfd36ia5quy63iqenba6ige724');
  assert.equal(binding.headers['Sora-Name'], 'docs.sora');
  assert.equal(binding.headers['Sora-Proof-Status'], 'ok');
  assert.match(binding.headers['Sora-Proof'], /^eyJhbGlhcyI6ImRvY3Muc29yYSIsIm1hbmlmZXN0IjoiYmFmeXI2/);
  assert.match(
    binding.headers['Sora-Route-Binding'],
    /^host=docs\.sora;cid=bafyr6icue6y6q7px7xiujin46u2aqwuqrfd36ia5quy63iqenba6ige724;generated_at=2026-03-10T12:00:00.000Z;label=prod$/,
  );
  assert.ok(binding.headers['Content-Security-Policy']);
  assert.ok(binding.headers['Strict-Transport-Security']);

  const tempDir = await mkdtemp(join(tmpdir(), 'gateway-binding-'));
  const headersOut = join(tempDir, 'headers.txt');
  const jsonOut = join(tempDir, 'binding.json');
  const generated = await generateGatewayBinding({
    manifestJsonPath: fixturePath,
    alias: 'docs.sora',
    hostname: 'docs.sora',
    proofStatus: 'ok',
    routeLabel: 'prod',
    now,
  });
  await writeFile(jsonOut, `${JSON.stringify(generated, null, 2)}\n`);
  await writeFile(headersOut, formatHeaders(generated.headers));

  const headerText = await readFile(headersOut, 'utf8');
  assert.match(headerText, /Sora-Content-CID: bafyr6icue6y6q7px7xiujin46u2aqwuqrfd36ia5quy63iqenba6ige724/);

  const jsonText = await readFile(jsonOut, 'utf8');
  const parsed = JSON.parse(jsonText);
  assert.equal(parsed.headers['Sora-Name'], 'docs.sora');
});
