// SPDX-License-Identifier: Apache-2.0

import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {
  access,
  chmod,
  link,
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rename,
  rm,
  stat,
  symlink,
  writeFile,
} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {promisify} from 'node:util';
import {fileURLToPath} from 'node:url';

import {
  OPENAPI_CARGO_LOCK_EXPECTED_BYTES,
  OPENAPI_CARGO_LOCK_EXPECTED_SHA256_HEX,
  OPENAPI_CARGO_LOCK_PIN_PATH,
  OPENAPI_CARGO_LOCK_PIN_SCHEMA,
  generateOpenApiCargoLockCandidate,
  parseArgs,
  parseOpenApiCargoLockPin,
  provisionOpenApiCargoLock,
  readOpenApiCargoLockStable,
  validateOpenApiCargoLockBytes,
} from '../provision-openapi-cargo-lock.mjs';

const execFileAsync = promisify(execFile);
const testDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(testDir, '..', '..', '..', '..');
const pinPath = path.join(repoRoot, OPENAPI_CARGO_LOCK_PIN_PATH);
const fixturePath = path.join(repoRoot, 'Cargo.lock');

let canonicalTempRoot;
let pinBytes;
let fixtureBytes;

test.before(async () => {
  canonicalTempRoot = await realpath(tmpdir());
  [pinBytes, fixtureBytes] = await Promise.all([
    readFile(pinPath),
    readFile(fixturePath),
  ]);
});

test('V1 pin and CLI are exact and canonical', async () => {
  const pin = parseOpenApiCargoLockPin(pinBytes);
  assert.deepEqual(pin, {
    schema: OPENAPI_CARGO_LOCK_PIN_SCHEMA,
    bytes: OPENAPI_CARGO_LOCK_EXPECTED_BYTES,
    sha256Hex: OPENAPI_CARGO_LOCK_EXPECTED_SHA256_HEX,
  });
  assert.equal(fixtureBytes.length, OPENAPI_CARGO_LOCK_EXPECTED_BYTES);
  assert.equal(
    validateOpenApiCargoLockBytes(fixtureBytes, pin),
    fixtureBytes,
  );
  assert.deepEqual(parseArgs([]), {sourcePath: undefined});
  assert.deepEqual(parseArgs(['/dev/null'].map((source) => `--source=${source}`)), {
    sourcePath: '/dev/null',
  });
  assert.throws(
    () => parseArgs(['--source=relative/Cargo.lock']),
    /absolute canonical path/i,
  );
  assert.throws(
    () => parseArgs(['--source=/a', '--source=/b']),
    /only once/i,
  );
  assert.throws(() => parseArgs(['--target=/tmp/Cargo.lock']), /unknown/i);

  for (const invalid of [
    pinBytes.subarray(0, pinBytes.length - 1),
    Buffer.from(
      pinBytes.toString('utf8').replace('bytes=315213', 'bytes=315214'),
    ),
    Buffer.from(
      pinBytes
        .toString('utf8')
        .replace(
          OPENAPI_CARGO_LOCK_EXPECTED_SHA256_HEX,
          '0'.repeat(64),
        ),
    ),
    Buffer.from(
      pinBytes
        .toString('utf8')
        .replace(OPENAPI_CARGO_LOCK_PIN_SCHEMA, 'iroha.openapi.cargo-lock.v2'),
    ),
  ]) {
    assert.throws(
      () => parseOpenApiCargoLockPin(invalid),
      /pin|schema|size|SHA-256|LF/i,
    );
  }
});

test(
  'stable reader rejects executable, linked, replaced, and mutated inputs',
  {skip: process.platform === 'win32'},
  async (context) => {
    const root = await makeTempDirectory('openapi-lock-stable-');
    context.after(() => rm(root, {recursive: true, force: true}));

    const executable = path.join(root, 'executable.lock');
    await writeFile(executable, 'lock');
    await chmod(executable, 0o755);
    await assert.rejects(
      () => readOpenApiCargoLockStable(executable),
      /must not be executable/i,
    );

    const linked = path.join(root, 'linked.lock');
    const hardLink = path.join(root, 'hard-link.lock');
    await writeFile(linked, 'lock');
    await link(linked, hardLink);
    await assert.rejects(
      () => readOpenApiCargoLockStable(linked),
      /exactly one hard link/i,
    );

    const symbolicLink = path.join(root, 'symbolic.lock');
    await symlink(executable, symbolicLink);
    await assert.rejects(
      () => readOpenApiCargoLockStable(symbolicLink),
      /symbolic link|without following links/i,
    );

    const replaced = path.join(root, 'replaced.lock');
    const replacedOld = path.join(root, 'replaced-old.lock');
    await writeFile(replaced, 'same-size');
    await assert.rejects(
      () =>
        readOpenApiCargoLockStable(replaced, {
          afterRead: async () => {
            await rename(replaced, replacedOld);
            await writeFile(replaced, 'same-size');
          },
        }),
      /replaced or mutated/i,
    );

    const mutated = path.join(root, 'mutated.lock');
    await writeFile(mutated, 'before');
    await assert.rejects(
      () =>
        readOpenApiCargoLockStable(mutated, {
          afterRead: () => writeFile(mutated, 'after!'),
        }),
      /replaced or mutated/i,
    );
  },
);

test('operator source installs atomically and exact root locks are reused', async (context) => {
  const root = await makeRepository({ignoreRootLock: true});
  context.after(() => rm(root, {recursive: true, force: true}));
  const source = await realpath(fixturePath);

  const installed = await provisionOpenApiCargoLock({
    repoRoot: root,
    sourcePath: source,
    generateCandidate: async () => {
      throw new Error('Cargo generation must not run with an operator source');
    },
  });
  assert.deepEqual(installed, {
    schema: 'iroha.openapi.cargo-lock.provision.v1',
    status: 'installed',
    source: 'operator',
    path: 'Cargo.lock',
    bytes: OPENAPI_CARGO_LOCK_EXPECTED_BYTES,
    sha256_hex: OPENAPI_CARGO_LOCK_EXPECTED_SHA256_HEX,
  });
  assert.deepEqual(await readFile(path.join(root, 'Cargo.lock')), fixtureBytes);
  if (process.platform !== 'win32') {
    assert.equal((await stat(path.join(root, 'Cargo.lock'))).mode & 0o777, 0o644);
  }
  assert.equal(await gitText(root, ['status', '--porcelain=v1']), '');

  const reused = await provisionOpenApiCargoLock({
    repoRoot: root,
    sourcePath: source,
    generateCandidate: async () => {
      throw new Error('Cargo generation must not run for an exact root lock');
    },
  });
  assert.equal(reused.status, 'reused');
  assert.equal(reused.source, 'existing');
});

test('fallback generation uses only an isolated external lockfile path', async (context) => {
  const root = await makeRepository({ignoreRootLock: true});
  context.after(() => rm(root, {recursive: true, force: true}));
  let generatedDirectory;

  const summary = await provisionOpenApiCargoLock({
    repoRoot: root,
    generateCandidate: async ({repoRoot: generatedRoot, candidatePath}) => {
      assert.equal(generatedRoot, root);
      assert.equal(path.basename(candidatePath), 'Cargo.lock');
      generatedDirectory = path.dirname(candidatePath);
      assert.equal(isWithin(root, candidatePath), false);
      await assert.rejects(
        () => access(path.join(root, 'Cargo.lock')),
        {code: 'ENOENT'},
      );
      await writeFile(candidatePath, fixtureBytes, {mode: 0o644});
    },
  });

  assert.equal(summary.status, 'installed');
  assert.equal(summary.source, 'generated');
  assert.deepEqual(await readFile(path.join(root, 'Cargo.lock')), fixtureBytes);
  await assert.rejects(() => access(generatedDirectory), {code: 'ENOENT'});
});

test(
  'Cargo fallback uses only unstable generate-lockfile with an external path',
  {skip: process.platform === 'win32'},
  async (context) => {
    const root = await makeTempDirectory('openapi-lock-cargo-command-');
    context.after(() => rm(root, {recursive: true, force: true}));
    const candidateDirectory = path.join(root, 'candidate');
    await mkdir(candidateDirectory);
    const candidatePath = path.join(candidateDirectory, 'Cargo.lock');
    const invocationPath = path.join(root, 'invocation.json');
    const fakeCargo = path.join(root, 'fake-cargo.mjs');
    await writeFile(
      fakeCargo,
      [
        '#!/usr/bin/env node',
        "import {writeFileSync} from 'node:fs';",
        `writeFileSync(${JSON.stringify(invocationPath)}, JSON.stringify({`,
        '  arguments: process.argv.slice(2),',
        '  rustcBootstrap: process.env.RUSTC_BOOTSTRAP,',
        '}));',
        '',
      ].join('\n'),
    );
    await chmod(fakeCargo, 0o700);

    await generateOpenApiCargoLockCandidate({
      repoRoot: root,
      candidatePath,
      cargoExecutable: fakeCargo,
    });
    assert.deepEqual(JSON.parse(await readFile(invocationPath, 'utf8')), {
      arguments: [
        '-Z',
        'unstable-options',
        'generate-lockfile',
        '--manifest-path',
        path.join(root, 'Cargo.toml'),
        '--lockfile-path',
        candidatePath,
      ],
      rustcBootstrap: '1',
    });
  },
);

test('wrong-size and wrong-digest candidates never install', async (context) => {
  for (const candidate of [
    fixtureBytes.subarray(0, fixtureBytes.length - 1),
    Buffer.concat([
      Buffer.from([fixtureBytes[0] ^ 0xff]),
      fixtureBytes.subarray(1),
    ]),
  ]) {
    const root = await makeRepository({ignoreRootLock: true});
    context.after(() => rm(root, {recursive: true, force: true}));
    const sourceDirectory = await makeTempDirectory('openapi-lock-invalid-');
    context.after(() => rm(sourceDirectory, {recursive: true, force: true}));
    const source = path.join(sourceDirectory, 'Cargo.lock');
    await writeFile(source, candidate);

    await assert.rejects(
      () =>
        provisionOpenApiCargoLock({
          repoRoot: root,
          sourcePath: source,
        }),
      /expected exactly|SHA-256/i,
    );
    await assert.rejects(
      () => access(path.join(root, 'Cargo.lock')),
      {code: 'ENOENT'},
    );
  }
});

test('an invalid existing root lock is rejected without replacement', async (context) => {
  const root = await makeRepository({ignoreRootLock: true});
  context.after(() => rm(root, {recursive: true, force: true}));
  const target = path.join(root, 'Cargo.lock');
  const invalid = Buffer.from('invalid existing lock\n');
  await writeFile(target, invalid);

  await assert.rejects(
    () =>
      provisionOpenApiCargoLock({
        repoRoot: root,
        sourcePath: fixturePath,
      }),
    /expected exactly/i,
  );
  assert.deepEqual(await readFile(target), invalid);
});

test('tracked and unignored root locks are rejected', async (context) => {
  const unignored = await makeRepository({ignoreRootLock: false});
  context.after(() => rm(unignored, {recursive: true, force: true}));
  await assert.rejects(
    () =>
      provisionOpenApiCargoLock({
        repoRoot: unignored,
        sourcePath: fixturePath,
      }),
    /must be ignored/i,
  );

  const tracked = await makeRepository({
    ignoreRootLock: true,
    trackRootLock: true,
  });
  context.after(() => rm(tracked, {recursive: true, force: true}));
  await assert.rejects(
    () => provisionOpenApiCargoLock({repoRoot: tracked}),
    /must remain untracked/i,
  );
});

test('staged Cargo.lock pin substitution is rejected', async (context) => {
  const root = await makeRepository({ignoreRootLock: true});
  context.after(() => rm(root, {recursive: true, force: true}));
  const repositoryPinPath = path.join(
    root,
    OPENAPI_CARGO_LOCK_PIN_PATH,
  );
  await writeFile(
    repositoryPinPath,
    pinBytes.toString('utf8').replace(
      OPENAPI_CARGO_LOCK_EXPECTED_SHA256_HEX,
      '0'.repeat(64),
    ),
  );
  await gitText(root, [
    'add',
    '--',
    OPENAPI_CARGO_LOCK_PIN_PATH,
  ]);
  await writeFile(repositoryPinPath, pinBytes);

  await assert.rejects(
    () =>
      provisionOpenApiCargoLock({
        repoRoot: root,
        generateCandidate: async () => {
          throw new Error('generation must not run for a substituted pin');
        },
      }),
    /pin index and HEAD entries must reference the same blob/i,
  );
});

test(
  'operator sources reject aliases, hard links, and executable files',
  {skip: process.platform === 'win32'},
  async (context) => {
    const root = await makeRepository({ignoreRootLock: true});
    context.after(() => rm(root, {recursive: true, force: true}));
    const sourceDirectory = await makeTempDirectory('openapi-lock-source-');
    context.after(() => rm(sourceDirectory, {recursive: true, force: true}));
    const source = path.join(sourceDirectory, 'Cargo.lock');
    await writeFile(source, fixtureBytes);

    const alias = path.join(sourceDirectory, 'alias.lock');
    await symlink(source, alias);
    await assert.rejects(
      () =>
        provisionOpenApiCargoLock({
          repoRoot: root,
          sourcePath: alias,
        }),
      /must be canonical/i,
    );

    const hardLink = path.join(sourceDirectory, 'hard-link.lock');
    await link(source, hardLink);
    await assert.rejects(
      () =>
        provisionOpenApiCargoLock({
          repoRoot: root,
          sourcePath: source,
        }),
      /exactly one hard link/i,
    );
    await rm(hardLink);

    await chmod(source, 0o755);
    await assert.rejects(
      () =>
        provisionOpenApiCargoLock({
          repoRoot: root,
          sourcePath: source,
        }),
      /must not be executable/i,
    );
  },
);

test('candidate mutation and target replacement fail closed', async (context) => {
  const mutationRoot = await makeRepository({ignoreRootLock: true});
  context.after(() => rm(mutationRoot, {recursive: true, force: true}));
  const sourceDirectory = await makeTempDirectory('openapi-lock-race-');
  context.after(() => rm(sourceDirectory, {recursive: true, force: true}));
  const source = path.join(sourceDirectory, 'Cargo.lock');
  await writeFile(source, fixtureBytes);

  await assert.rejects(
    () =>
      provisionOpenApiCargoLock({
        repoRoot: mutationRoot,
        sourcePath: source,
        beforeInstall: async () => {
          const mutated = Buffer.from(fixtureBytes);
          mutated[0] ^= 0xff;
          await writeFile(source, mutated);
        },
      }),
    /replaced or mutated after read/i,
  );
  await assert.rejects(
    () => access(path.join(mutationRoot, 'Cargo.lock')),
    {code: 'ENOENT'},
  );

  const replacementRoot = await makeRepository({ignoreRootLock: true});
  context.after(() => rm(replacementRoot, {recursive: true, force: true}));
  await writeFile(source, fixtureBytes);
  await assert.rejects(
    () =>
      provisionOpenApiCargoLock({
        repoRoot: replacementRoot,
        sourcePath: source,
        beforeInstall: () =>
          writeFile(path.join(replacementRoot, 'Cargo.lock'), fixtureBytes),
      }),
    /appeared or was replaced/i,
  );
});

test('OpenAPI workflow provisions before both release gates and stays clean', async () => {
  const workflow = await readFile(
    path.join(repoRoot, '.github', 'workflows', 'openapi.yml'),
    'utf8',
  );
  assert.equal(
    Array.from(
      workflow.matchAll(/release\/openapi-cargo-lock-v1\.txt/g),
    ).length,
    2,
  );
  assert.equal(
    Array.from(
      workflow.matchAll(
        /node tools\/openapi\/scripts\/provision-openapi-cargo-lock\.mjs/g,
      ),
    ).length,
    2,
  );
  assert.equal(
    Array.from(
      workflow.matchAll(
        /git status --porcelain=v1 --untracked-files=all/g,
      ),
    ).length,
    2,
  );
  assert.equal(
    Array.from(
      workflow.matchAll(/npm ci --ignore-scripts --no-audit --no-fund/g),
    ).length,
    2,
  );
  assert.match(workflow, /provision-openapi-cargo-lock\.test\.mjs/);

  const metadataStart = workflow.indexOf('  metadata:');
  const canonicalStart = workflow.indexOf('  canonical-spec:');
  const metadata = workflow.slice(metadataStart, canonicalStart);
  const canonical = workflow.slice(canonicalStart);
  assert.ok(
    metadata.indexOf('provision-openapi-cargo-lock.mjs') <
      metadata.indexOf('verify-openapi-release-inputs.mjs'),
  );
  assert.ok(
    canonical.indexOf('provision-openapi-cargo-lock.mjs') <
      canonical.indexOf('bash ci/check_openapi_spec.sh'),
  );
  assert.ok(
    canonical.indexOf('npm ci --ignore-scripts --no-audit --no-fund') <
      canonical.indexOf('bash ci/check_openapi_spec.sh'),
  );
});

async function makeTempDirectory(prefix) {
  return mkdtemp(path.join(canonicalTempRoot, prefix));
}

async function makeRepository({
  ignoreRootLock,
  trackRootLock = false,
}) {
  const root = await makeTempDirectory('openapi-lock-repository-');
  await mkdir(path.join(root, 'release'));
  await Promise.all([
    writeFile(
      path.join(root, 'Cargo.toml'),
      '[workspace]\nmembers = []\nresolver = "2"\n',
    ),
    writeFile(path.join(root, OPENAPI_CARGO_LOCK_PIN_PATH), pinBytes),
    writeFile(
      path.join(root, '.gitignore'),
      ignoreRootLock ? 'Cargo.lock\n' : '# Cargo.lock is not ignored\n',
    ),
  ]);
  if (trackRootLock) {
    await writeFile(path.join(root, 'Cargo.lock'), fixtureBytes);
  }
  await gitText(root, ['init', '--quiet']);
  await gitText(root, ['config', 'user.email', 'openapi-test@example.invalid']);
  await gitText(root, ['config', 'user.name', 'OpenAPI Test']);
  await gitText(root, ['add', '.']);
  if (trackRootLock) {
    await gitText(root, ['add', '--force', 'Cargo.lock']);
  }
  await gitText(root, ['commit', '--quiet', '-m', 'fixture']);
  return realpath(root);
}

async function gitText(root, arguments_) {
  const {stdout} = await execFileAsync('git', arguments_, {
    cwd: root,
    encoding: 'utf8',
  });
  return stdout;
}

function isWithin(parent, child) {
  const relative = path.relative(parent, child);
  return relative === '' || (
    relative !== '..' &&
    !relative.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relative)
  );
}
