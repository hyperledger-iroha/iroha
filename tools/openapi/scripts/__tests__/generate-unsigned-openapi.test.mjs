import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {chmod, cp, mkdir, mkdtemp, readFile, readdir, realpath, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {promisify} from 'node:util';
import test from 'node:test';

import {generateUnsignedOpenApi} from '../generate-unsigned-openapi.mjs';
import {
  OPENAPI_GENERATOR_INPUT_INVENTORY_HEADER, OPENAPI_GENERATOR_INPUT_PATHS,
  captureOpenApiGeneratorSource, verifyOpenApiReleaseInputs,
} from '../verify-openapi-release-inputs.mjs';
import {encodeOpenApiCargoLockPin, isolateGitRepositoryEnvironment} from '../provision-openapi-cargo-lock.mjs';

const execFileAsync = promisify(execFile);
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../../../..');
const generatorPath = 'tools/openapi/scripts/generate-unsigned-openapi.mjs';
const specPaths = [
  'artifacts/openapi/torii.json', 'crates/iroha_torii/assets/openapi/torii.json',
  'artifacts/openapi/versions/current/torii.json',
];
const spec = Buffer.from(`${JSON.stringify({
  openapi: '3.1.0', info: {title: 'fixture', version: '1'},
  paths: {'/health': {get: {responses: {'200': {description: 'ok'}}}}},
  components: {schemas: {Health: {type: 'object'}}},
})}\n`);
async function git(root, args) {
  const result = await execFileAsync('git', ['-C', root, ...args], {
    env: isolateGitRepositoryEnvironment(), maxBuffer: 128 * 1024 * 1024,
  });
  return result.stdout.trim();
}
async function commit(root) {
  await git(root, ['add', '-A']);
  await git(root, ['commit', '--quiet', '-m', 'OpenAPI fixture checkpoint']);
  return git(root, ['rev-parse', 'HEAD']);
}
async function fixture(t) {
  const parent = await realpath(await mkdtemp(join(tmpdir(), 'openapi-authored-test-')));
  t.after(() => rm(parent, {recursive: true, force: true}));
  await chmod(parent, 0o700);
  const root = join(parent, 'source');
  const output = join(parent, 'output');
  await mkdir(root, {mode: 0o700});
  await mkdir(output, {mode: 0o700});
  await git(root, ['init', '--quiet']);
  await git(root, ['config', 'user.email', 'openapi-test@example.invalid']);
  await git(root, ['config', 'user.name', 'OpenAPI Fixture']);
  const lock = Buffer.from('# fixture Cargo.lock\nversion = 4\n');
  for (const path of OPENAPI_GENERATOR_INPUT_PATHS.filter((candidate) =>
    !OPENAPI_GENERATOR_INPUT_PATHS.some((other) => other.startsWith(`${candidate}/`)))) {
    const actual = ['tools', 'crates'].includes(path) ? `${path}/fixture-input.txt` : path;
    let bytes = Buffer.from(`fixture ${actual}\n`);
    if (path === 'Cargo.lock') bytes = lock;
    if (path === 'release/openapi-cargo-lock-v1.txt') bytes = encodeOpenApiCargoLockPin(lock);
    if (path === 'release/openapi-generator-inputs-v1.txt') {
      bytes = Buffer.from(`${OPENAPI_GENERATOR_INPUT_INVENTORY_HEADER}\n${OPENAPI_GENERATOR_INPUT_PATHS.join('\n')}\n`);
    }
    if (path === 'artifacts/openapi/allowed_signers.json') bytes = Buffer.from('{"version":1,"allow":[]}\n');
    await mkdir(dirname(join(root, actual)), {recursive: true});
    await writeFile(join(root, actual), bytes);
  }
  for (const path of [...specPaths, generatorPath]) {
    await mkdir(dirname(join(root, path)), {recursive: true});
    await writeFile(join(root, path), path === generatorPath ? await readFile(join(repoRoot, generatorPath)) : spec);
  }
  const expectedCommit = await commit(root);
  return {root, output, expectedCommit, options: {sourceRoot: root, outputDir: output, expectedCommit}};
}
async function assertUnpublished(f) {
  assert.deepEqual(await readdir(f.output), []);
  assert.equal((await readdir(dirname(f.output))).some((name) => name.startsWith('.openapi-authored-')), false);
}

test('authored unsigned generation roundtrips the real verifier and an output-only commit', async (t) => {
  const f = await fixture(t);
  const receipt = await generateUnsignedOpenApi(f.options);
  assert.equal(receipt.schema, 'iroha.openapi.unsigned_authored_spec.v1');
  assert.equal(receipt.runtime_projection, 'not_executed');
  assert.equal(receipt.candidate_commit, f.expectedCommit);
  assert.equal(receipt.generator_commit, f.expectedCommit);
  assert.equal(receipt.generated_unix_ms, Number(await git(f.root, ['show', '-s', '--format=%ct', 'HEAD'])) * 1000);
  const manifest = JSON.parse(await readFile(join(f.output, 'manifest.json')));
  assert.equal(manifest.artifact.signature, null);
  assert.equal(manifest.generator_dirty, false);
  assert.deepEqual(await readFile(join(f.output, 'torii.json')), spec);
  assert.deepEqual(await readFile(join(f.root, specPaths[1])), spec);
  assert.equal(await git(f.root, ['status', '--porcelain']), '');
  const before = await verifyOpenApiReleaseInputs({repoRoot: f.root, outputDir: f.output});
  for (const path of ['torii.json', 'manifest.json', 'versions/current/torii.json', 'versions/current/manifest.json', 'versions.json']) {
    await cp(join(f.output, path), join(f.root, 'artifacts/openapi', path));
  }
  await commit(f.root);
  assert.deepEqual(await verifyOpenApiReleaseInputs({repoRoot: f.root}), before);
  const second = join(dirname(f.output), 'second');
  await mkdir(second, {mode: 0o700});
  await generateUnsignedOpenApi({...f.options, outputDir: second, expectedCommit: await git(f.root, ['rev-parse', 'HEAD'])});
});

test('wrong source commit and dirty source cannot publish metadata', async (t) => {
  const f = await fixture(t);
  await assert.rejects(generateUnsignedOpenApi({...f.options, expectedCommit: '11'.repeat(20)}), /exact expected clean HEAD/);
  await assertUnpublished(f);
  await writeFile(join(f.root, 'Cargo.toml'), 'dirty source\n');
  await assert.rejects(generateUnsignedOpenApi(f.options), /checkout changed/);
  await assertUnpublished(f);
});

test('all three exact authored copies must agree at the clean source commit', async (t) => {
  const f = await fixture(t);
  await writeFile(join(f.root, specPaths[1]), Buffer.concat([spec, Buffer.from('\n')]));
  f.options.expectedCommit = await commit(f.root);
  await assert.rejects(generateUnsignedOpenApi(f.options), /byte-identical/);
  await assertUnpublished(f);
});

test('source and staged-byte mutation before completion leave no partial publication', async (t) => {
  for (const kind of ['source', 'staged']) {
    const f = await fixture(t);
    await assert.rejects(generateUnsignedOpenApi({...f.options, beforeFinalStateCheck: async ({stage}) => {
      await writeFile(kind === 'source' ? join(f.root, 'Cargo.toml') : join(stage, 'torii.json'), 'mutation\n');
    }}), kind === 'source' ? /clean checkout/ : /byte-identical|JSON|specification/);
    await assertUnpublished(f);
  }
});

test('destination mutation is preserved and prevents publication', async (t) => {
  const f = await fixture(t);
  await assert.rejects(generateUnsignedOpenApi({...f.options, beforeFinalStateCheck: async () => {
    await writeFile(join(f.output, 'other-owner.txt'), 'preserve\n');
  }}), /must be empty/);
  assert.deepEqual(await readdir(f.output), ['other-owner.txt']);
  assert.equal(await readFile(join(f.output, 'other-owner.txt'), 'utf8'), 'preserve\n');
});

test('recursive tools inventory binds generator implementation changes', async (t) => {
  const f = await fixture(t);
  const first = await captureOpenApiGeneratorSource({repoRoot: f.root, expectedCommit: f.expectedCommit});
  await writeFile(join(f.root, generatorPath), '// changed generator source\n');
  const nextCommit = await commit(f.root);
  const second = await captureOpenApiGeneratorSource({repoRoot: f.root, expectedCommit: nextCommit});
  assert.notEqual(first.sourceSha256Hex, second.sourceSha256Hex);
  await assert.rejects(first.assertUnchanged(), /HEAD changed/);
});

test('invalid output and hooks fail without publishing or overwriting', async (t) => {
  const f = await fixture(t);
  await assert.rejects(generateUnsignedOpenApi({...f.options, outputDir: 'relative'}), /absolute canonical/);
  await assert.rejects(generateUnsignedOpenApi({...f.options, outputDir: f.root}), /outside the source/);
  await assert.rejects(generateUnsignedOpenApi({...f.options, outputDir: join(f.root, '..still-source')}), /outside the source/);
  await assert.rejects(generateUnsignedOpenApi({...f.options, beforeFinalStateCheck: true}), /must be a function/);
  await chmod(f.output, 0o755);
  await assert.rejects(generateUnsignedOpenApi(f.options), /owner-private/);
  await assertUnpublished(f);
});

test('plain unsigned CI selects the Node owner before any clone or Cargo target', async () => {
  const wrapper = await readFile(join(repoRoot, 'ci/run_openapi_generator.sh'), 'utf8');
  const branch = wrapper.indexOf('if [[ "${UNSIGNED_MANIFEST}" == 1 && -z "${SIGNING_PAYLOAD}" ]]');
  const end = wrapper.indexOf('\nfi', branch);
  assert.ok(branch > 0 && end > branch);
  assert.match(wrapper.slice(branch, end), /generate-unsigned-openapi\.mjs/);
  assert.match(wrapper.slice(branch, end), /exit 0/);
  assert.ok(end < wrapper.indexOf('OPENAPI_RUN_ROOT="$(mktemp'));
  assert.ok(end < wrapper.indexOf('git clone --quiet'));
  assert.match(wrapper.slice(end), /--features dev-tools/);
});

test('committed historical versions and unchanged timestamps survive regeneration', async (t) => {
  const f = await fixture(t);
  await generateUnsignedOpenApi(f.options);
  const historical = join(f.root, 'artifacts/openapi/versions/release1');
  await mkdir(historical, {recursive: true});
  for (const name of ['torii.json', 'manifest.json']) {
    await cp(join(f.output, name), join(historical, name));
  }
  const index = JSON.parse(await readFile(join(f.output, 'versions.json')));
  const historyEntry = {...index.entries[1], label: 'release1',
    path: 'versions/release1/torii.json', manifestPath: 'versions/release1/manifest.json',
    updatedAt: '2001-01-01T00:00:00.000Z'};
  index.entries.push(historyEntry);
  index.versions.push('release1');
  await writeFile(join(f.root, 'artifacts/openapi/versions.json'), JSON.stringify(index));
  const sourceCommit = await commit(f.root);
  const next = join(dirname(f.output), 'next');
  await mkdir(next, {mode: 0o700});
  await generateUnsignedOpenApi({...f.options, expectedCommit: sourceCommit, outputDir: next});
  const actual = JSON.parse(await readFile(join(next, 'versions.json')));
  assert.deepEqual(actual.entries.find((entry) => entry.label === 'release1'), historyEntry);
  assert.deepEqual(await readFile(join(next, 'versions/release1/manifest.json')), await readFile(join(historical, 'manifest.json')));

  // Git-clean sparse materialization must not silently remove committed history.
  await git(f.root, ['update-index', '--skip-worktree',
    'artifacts/openapi/versions/release1/torii.json',
    'artifacts/openapi/versions/release1/manifest.json']);
  await rm(historical, {recursive: true});
  assert.equal(await git(f.root, ['status', '--porcelain']), '');
  const sparseOutput = join(dirname(f.output), 'sparse-output');
  await mkdir(sparseOutput, {mode: 0o700});
  await assert.rejects(generateUnsignedOpenApi({...f.options, expectedCommit: sourceCommit, outputDir: sparseOutput}), /ENOENT|does not exist|unavailable/);
  assert.deepEqual(await readdir(sparseOutput), []);
});
