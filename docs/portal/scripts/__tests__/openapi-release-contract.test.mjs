import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const testDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(testDir, '..', '..', '..', '..');

test('OpenAPI CI uses a locked graph and detached-only signing', async () => {
  const gate = await readFile(join(repoRoot, 'ci', 'check_openapi_spec.sh'), 'utf8');

  assert.match(gate, /cargo run \\\n\s+--locked \\\n\s+--offline \\/);
  assert.doesNotMatch(gate, /(?:^|\s)--sign(?:\s|\\|$)/m);
  assert.match(gate, /--unsigned-manifest --signing-payload/);
  assert.match(gate, /--signature-envelope/);
});

test('OpenAPI CI is clean, commit-bound, and two-pass', async () => {
  const gate = await readFile(join(repoRoot, 'ci', 'check_openapi_spec.sh'), 'utf8');

  assert.match(gate, /require_clean_checkout/);
  assert.match(gate, /rev-parse --verify 'HEAD\^\{commit\}'/);
  assert.match(
    gate,
    /--expected-generator-commit="\$\{EXPECTED_GENERATOR_COMMIT\}"/,
  );
  assert.equal(
    Array.from(gate.matchAll(/run_xtask openapi --output/g)).length,
    2,
  );
  assert.match(
    gate,
    /diff -u "\$\{GENERATED_SPEC_FIRST\}" "\$\{GENERATED_SPEC_SECOND\}"/,
  );
  assert.match(
    gate,
    /diff -u "\$\{MANIFEST_PATH\}" "\$\{CURRENT_MANIFEST_PATH\}"/,
  );
  assert.doesNotMatch(gate, /VERSION_VERIFY_POLICY_ARGS/);
});

test('OpenAPI workflow actions and tooling tests are pinned', async () => {
  const workflow = await readFile(
    join(repoRoot, '.github', 'workflows', 'openapi.yml'),
    'utf8',
  );
  const actionReferences = Array.from(
    workflow.matchAll(/^\s*-\s+uses:\s+([^\s#]+)\s*$/gm),
    (match) => match[1],
  );

  assert.ok(actionReferences.length > 0);
  for (const reference of actionReferences) {
    assert.match(reference, /@[0-9a-f]{40}$/);
  }
  for (const testFile of [
    'openapi-manifest-v2.test.mjs',
    'openapi-release-contract.test.mjs',
    'openapi-safe-file.test.mjs',
  ]) {
    assert.match(workflow, new RegExp(testFile.replaceAll('.', '\\.')));
  }
});
