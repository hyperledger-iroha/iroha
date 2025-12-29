// Control-plane OpenAPI/RBAC snapshot parity (SNNet-15D)
import assert from 'node:assert';
import { test } from 'node:test';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function sha256(filePath) {
  const data = fs.readFileSync(filePath);
  const h = createHash('sha256');
  h.update(data);
  return h.digest('hex');
}

const repoRoot = path.resolve(__dirname, '..', '..', '..');
const openapiPath = path.join(repoRoot, 'docs', 'source', 'soranet', 'control_plane_openapi.yaml');
const rbacPath = path.join(repoRoot, 'docs', 'source', 'soranet', 'control_plane_rbac.yaml');

const expected = {
  openapi: '1c8a52055aa687bf41221908491eb92e909402f5a51814ea8befb5ccaeb923b6',
  rbac: '1ef5e444101ac37bcbd704e5abb80dc714fac16c686b27d9c419e0e47bca63ea',
};

test('control-plane snapshots (OpenAPI/RBAC parity)', () => {
  assert.strictEqual(sha256(openapiPath), expected.openapi);
  assert.strictEqual(sha256(rbacPath), expected.rbac);
});
