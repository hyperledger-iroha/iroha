// SPDX-License-Identifier: Apache-2.0

import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {createHash} from 'node:crypto';
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
  symlink,
  writeFile,
} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {promisify} from 'node:util';
import {fileURLToPath} from 'node:url';

import {
  OPENAPI_CARGO_LOCK_PIN_OWNER_SCHEMA,
  OPENAPI_CARGO_LOCK_PIN_PATH,
  OPENAPI_CARGO_LOCK_PIN_SCHEMA,
  encodeOpenApiCargoLockPin,
  generateOpenApiCargoLockPin,
  isolateGitRepositoryEnvironment,
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

let canonicalTempRoot;
let fixtureDirectory;
let fixturePath;
let pinBytes;
let fixtureBytes;
let pin;

test.before(async () => {
  canonicalTempRoot = await realpath(tmpdir());
  const trackedPinBytes = await readFile(pinPath);
  parseOpenApiCargoLockPin(trackedPinBytes);
  fixtureBytes = Buffer.from(
    '# synthetic OpenAPI Cargo.lock fixture\nversion = 4\n',
    'utf8',
  );
  pinBytes = encodeOpenApiCargoLockPin(fixtureBytes);
  pin = parseOpenApiCargoLockPin(pinBytes);
  fixtureDirectory = await makeTempDirectory('openapi-lock-canonical-');
  fixturePath = path.join(fixtureDirectory, 'Cargo.lock');
  await writeFile(fixturePath, fixtureBytes, {mode: 0o644});
});

test.after(async () => {
  if (fixtureDirectory) {
    await rm(fixtureDirectory, {recursive: true, force: true});
  }
});

test('V1 pin and CLI are exact and canonical', async () => {
  assert.deepEqual(pin, {
    schema: OPENAPI_CARGO_LOCK_PIN_SCHEMA,
    bytes: fixtureBytes.length,
    sha256Hex: createHash('sha256').update(fixtureBytes).digest('hex'),
  });
  assert.equal(
    validateOpenApiCargoLockBytes(fixtureBytes, pin),
    fixtureBytes,
  );
  assert.deepEqual(parseArgs(['provision']), {
    command: 'provision',
    sourcePath: undefined,
  });
  assert.deepEqual(parseArgs(['provision', '--source=/dev/null']), {
    command: 'provision',
    sourcePath: '/dev/null',
  });
  assert.throws(() => parseArgs([]), /requires a provision or pin command/i);
  assert.throws(
    () => parseArgs(['provision', '--source=relative/Cargo.lock']),
    /absolute canonical path/i,
  );
  assert.throws(
    () => parseArgs(['provision', '--source=/a', '--source=/b']),
    /only once/i,
  );
  assert.throws(
    () => parseArgs(['provision', '--target=/tmp/Cargo.lock']),
    /unknown/i,
  );
  assert.throws(() => parseArgs(['--source=/dev/null']), /unknown.*command/i);

  for (const invalid of [
    pinBytes.subarray(0, pinBytes.length - 1),
    Buffer.from(
      pinBytes.toString('utf8').replace(`bytes=${pin.bytes}`, 'bytes=0'),
    ),
    Buffer.from(
      pinBytes
        .toString('utf8')
        .replace(pin.sha256Hex, '0'.repeat(64)),
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

test('Git environment isolation removes routing, object, and config inputs', () => {
  const isolated = isolateGitRepositoryEnvironment({
    PATH: '/safe/bin',
    GIT_DIR: '/sentinel/.git',
    GIT_INDEX_FILE: '/sentinel/index',
    GIT_OBJECT_DIRECTORY: '/sentinel/objects',
    GIT_ALTERNATE_OBJECT_DIRECTORIES: '/sentinel/alternate',
    GIT_CONFIG_COUNT: '1',
    GIT_CONFIG_KEY_0: 'core.fsmonitor',
    GIT_CONFIG_VALUE_0: 'malicious',
  });
  assert.deepEqual(isolated, {
    PATH: '/safe/bin',
    GIT_OPTIONAL_LOCKS: '0',
    GIT_NO_LAZY_FETCH: '1',
    GIT_NO_REPLACE_OBJECTS: '1',
    GIT_CONFIG_NOSYSTEM: '1',
    GIT_CONFIG_GLOBAL: '/dev/null',
    GIT_CONFIG_COUNT: '2',
    GIT_CONFIG_KEY_0: 'core.hooksPath',
    GIT_CONFIG_VALUE_0: '/dev/null',
    GIT_CONFIG_KEY_1: 'core.fsmonitor',
    GIT_CONFIG_VALUE_1: 'false',
  });
});

test('Git environment isolation disables repository-local replace refs', async (context) => {
  const root = await makeTempDirectory('openapi-lock-replace-ref-');
  context.after(() => rm(root, {recursive: true, force: true}));
  const valuePath = path.join(root, 'value.txt');
  await gitText(root, ['init', '--quiet']);
  await gitText(root, ['config', 'user.email', 'openapi-test@example.invalid']);
  await gitText(root, ['config', 'user.name', 'OpenAPI Test']);
  await writeFile(valuePath, 'original\n');
  await gitText(root, ['add', 'value.txt']);
  await gitText(root, ['commit', '--quiet', '-m', 'original']);
  const originalBlob = (await gitText(root, ['rev-parse', 'HEAD:value.txt'])).trim();
  await writeFile(valuePath, 'replacement\n');
  await gitText(root, ['add', 'value.txt']);
  await gitText(root, ['commit', '--quiet', '-m', 'replacement']);
  const replacementBlob = (await gitText(root, ['rev-parse', 'HEAD:value.txt'])).trim();
  await gitText(root, [
    'update-ref',
    `refs/replace/${originalBlob}`,
    replacementBlob,
  ]);

  const replacementEnabled = isolateGitRepositoryEnvironment();
  delete replacementEnabled.GIT_NO_REPLACE_OBJECTS;
  const replaced = await execFileAsync(
    'git',
    ['cat-file', 'blob', originalBlob],
    {cwd: root, encoding: 'utf8', env: replacementEnabled},
  );
  assert.equal(replaced.stdout, 'replacement\n');
  assert.equal(
    await gitText(root, ['cat-file', 'blob', originalBlob]),
    'original\n',
  );
});

test('pin-owner CLI requires one absolute source and one explicit mode', () => {
  const root = path.parse(process.cwd()).root;
  const sourcePath = path.join(root, 'source', 'Cargo.lock');
  const outputPath = path.join(root, 'stage', 'pin.txt');
  const checkPath = path.join(root, 'repo', 'pin.txt');
  assert.deepEqual(
    parseArgs(['pin', `--source=${sourcePath}`, `--output=${outputPath}`]),
    {command: 'pin', sourcePath, outputPath, checkPath: undefined},
  );
  assert.deepEqual(
    parseArgs(['pin', `--source=${sourcePath}`, `--check=${checkPath}`]),
    {command: 'pin', sourcePath, outputPath: undefined, checkPath},
  );
  assert.throws(() => parseArgs(['pin']), /requires explicit --source/i);
  assert.throws(
    () => parseArgs(['pin', '--source=relative/Cargo.lock', `--output=${outputPath}`]),
    /absolute normalized/i,
  );
  assert.throws(
    () => parseArgs(['pin', `--source=${sourcePath}`, '--output=relative/pin.txt']),
    /absolute normalized/i,
  );
  assert.throws(
    () =>
      parseArgs([
        'pin',
        `--source=${sourcePath}`,
        `--output=${outputPath}`,
        `--check=${checkPath}`,
      ]),
    /exactly one/i,
  );
  assert.throws(
    () => parseArgs(['pin', `--source=${sourcePath}`, `--output=${sourcePath}`]),
    /must not alias/i,
  );
  assert.throws(
    () => parseArgs(['pin', `--source=${sourcePath}`, '--output-root=/tmp/pin']),
    /unknown/i,
  );
});

test('pin owner stages canonical bytes idempotently and verifies them', async (context) => {
  const root = await makeTempDirectory('openapi-pin-owner-');
  context.after(() => rm(root, {recursive: true, force: true}));
  const sourcePath = path.join(root, 'Cargo.lock');
  const outputPath = path.join(root, 'openapi-cargo-lock-v1.txt');
  const sourceBytes = Buffer.from('# deterministic lock fixture\nversion = 4\n');
  await writeFile(sourcePath, sourceBytes, {mode: 0o644});
  const trackedBefore = await readFile(pinPath);

  const first = await generateOpenApiCargoLockPin({sourcePath, outputPath});
  const firstBytes = await readFile(outputPath);
  assert.deepEqual(firstBytes, encodeOpenApiCargoLockPin(sourceBytes));
  assert.equal(first.schema, OPENAPI_CARGO_LOCK_PIN_OWNER_SCHEMA);
  assert.equal(first.status, 'staged');
  assert.equal(first.source, sourcePath);
  assert.equal(first.path, outputPath);

  const second = await generateOpenApiCargoLockPin({sourcePath, outputPath});
  assert.equal(second.status, 'staged');
  assert.deepEqual(await readFile(outputPath), firstBytes);

  const verified = await generateOpenApiCargoLockPin({
    sourcePath,
    checkPath: outputPath,
  });
  assert.equal(verified.status, 'verified');
  assert.deepEqual(await readFile(pinPath), trackedBefore);
});

test(
  'pin owner rejects stale checks, linked sources, and executable sources',
  {skip: process.platform === 'win32'},
  async (context) => {
    const root = await makeTempDirectory('openapi-pin-owner-invalid-');
    context.after(() => rm(root, {recursive: true, force: true}));
    const sourcePath = path.join(root, 'Cargo.lock');
    const checkPath = path.join(root, 'pin.txt');
    await writeFile(sourcePath, 'source lock\n', {mode: 0o644});
    await writeFile(checkPath, encodeOpenApiCargoLockPin(Buffer.from('other lock\n')));
    await assert.rejects(
      () => generateOpenApiCargoLockPin({sourcePath, checkPath}),
      /is stale/i,
    );

    const symbolicPath = path.join(root, 'symbolic.lock');
    await symlink(sourcePath, symbolicPath);
    await assert.rejects(
      () => generateOpenApiCargoLockPin({sourcePath: symbolicPath, checkPath}),
      /must be canonical|symbolic link/i,
    );

    const hardLinkPath = path.join(root, 'hard-link.lock');
    await link(sourcePath, hardLinkPath);
    await assert.rejects(
      () => generateOpenApiCargoLockPin({sourcePath, checkPath}),
      /exactly one hard link/i,
    );
    await rm(hardLinkPath);

    await chmod(sourcePath, 0o755);
    await assert.rejects(
      () => generateOpenApiCargoLockPin({sourcePath, checkPath}),
      /must not be executable/i,
    );
  },
);

test('pin owner detects source races and refuses repository outputs', async (context) => {
  const root = await makeTempDirectory('openapi-pin-owner-race-');
  context.after(() => rm(root, {recursive: true, force: true}));
  const sourcePath = path.join(root, 'Cargo.lock');
  const outputPath = path.join(root, 'pin.txt');
  await writeFile(sourcePath, 'before\n', {mode: 0o644});

  await assert.rejects(
    () =>
      generateOpenApiCargoLockPin({
        sourcePath,
        outputPath,
        beforePublish: () => writeFile(sourcePath, 'after!\n'),
      }),
    /replaced or mutated after read/i,
  );
  await assert.rejects(() => access(outputPath), {code: 'ENOENT'});

  await writeFile(sourcePath, 'stable\n', {mode: 0o644});
  const repositoryOutput = path.join(repoRoot, 'release', 'pin-owner-must-not-write.txt');
  await assert.rejects(
    () => generateOpenApiCargoLockPin({sourcePath, outputPath: repositoryOutput}),
    /outside the repository/i,
  );
  await assert.rejects(() => access(repositoryOutput), {code: 'ENOENT'});
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

    const fifoRace = path.join(root, 'fifo-race.lock');
    const fifoOriginal = path.join(root, 'fifo-race-original.lock');
    await writeFile(fifoRace, 'lock');
    await assert.rejects(
      () =>
        readOpenApiCargoLockStable(fifoRace, {
          beforeOpen: async () => {
            await rename(fifoRace, fifoOriginal);
            await execFileAsync('mkfifo', [fifoRace]);
          },
        }),
      /must be a regular file/i,
    );

    const oversizedPin = path.join(root, 'oversized-pin');
    await writeFile(oversizedPin, Buffer.alloc(1025, 1));
    await assert.rejects(
      () => readOpenApiCargoLockStable(oversizedPin, {maxBytes: 1024}),
      /exceeds the 1024-byte limit/i,
    );
    const special = path.join(root, 'special');
    await mkdir(special);
    await assert.rejects(
      () => readOpenApiCargoLockStable(special, {maxBytes: 1024}),
      /must be a regular file/i,
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

test('tracked root authority verifies without writing the checkout', async (context) => {
  const root = await makeRepository();
  context.after(() => rm(root, {recursive: true, force: true}));
  const target = path.join(root, 'Cargo.lock');
  const before = await readFile(target);
  let hookEvidence;

  const verified = await provisionOpenApiCargoLock({
    repoRoot: root,
    sourcePath: await realpath(fixturePath),
    beforeVerify: (evidence) => {
      hookEvidence = evidence;
    },
  });
  assert.deepEqual(verified, {
    schema: 'iroha.openapi.cargo-lock.provision.v1',
    status: 'verified',
    source: 'tracked',
    path: 'Cargo.lock',
    bytes: pin.bytes,
    sha256_hex: pin.sha256Hex,
  });
  assert.deepEqual(hookEvidence, {
    sourcePath: await realpath(fixturePath),
    trackedPath: target,
  });
  assert.deepEqual(await readFile(target), before);
  assert.equal(await gitText(root, ['status', '--porcelain=v1']), '');
});

test('an absent or untracked root lock fails closed without adopting a source', async (context) => {
  for (const withWorkingLock of [false, true]) {
    const root = await makeRepository({trackRootLock: false});
    context.after(() => rm(root, {recursive: true, force: true}));
    if (withWorkingLock) {
      await writeFile(path.join(root, 'Cargo.lock'), fixtureBytes);
    }
    let beforeVerifyCalled = false;
    await assert.rejects(
      () =>
        provisionOpenApiCargoLock({
          repoRoot: root,
          sourcePath: fixturePath,
          beforeVerify: () => {
            beforeVerifyCalled = true;
          },
        }),
      /stage-zero 100644 Git blob/i,
    );
    assert.equal(beforeVerifyCalled, false);
  }
});

test('provisioner has no Cargo execution or lock generation surface', async () => {
  const source = await readFile(
    path.join(testDir, '..', 'provision-openapi-cargo-lock.mjs'),
    'utf8',
  );
  for (const forbidden of [
    'generate-lockfile',
    '--lockfile-path',
    'unstable-options',
    'RUSTC_BOOTSTRAP',
    'cargoExecutable',
    'spawnChecked',
  ]) {
    assert.equal(source.includes(forbidden), false, forbidden);
  }
  assert.equal(Array.from(source.matchAll(/\bspawn\(/g)).length, 1);
  assert.match(source, /const child = spawn\('git', arguments_/);
  assert.doesNotMatch(source, /readFileSync|OPENAPI_CARGO_LOCK_EXPECTED_/);
});

test('wrong-size and wrong-digest comparison candidates never alter the authority', async (context) => {
  for (const candidate of [
    fixtureBytes.subarray(0, fixtureBytes.length - 1),
    Buffer.concat([
      Buffer.from([fixtureBytes[0] ^ 0xff]),
      fixtureBytes.subarray(1),
    ]),
  ]) {
    const root = await makeRepository();
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
    assert.deepEqual(await readFile(path.join(root, 'Cargo.lock')), fixtureBytes);
    assert.equal(await gitText(root, ['status', '--porcelain=v1']), '');
  }
});

test('root worktree, index, and mode substitutions are rejected', async (context) => {
  const dirty = await makeRepository();
  context.after(() => rm(dirty, {recursive: true, force: true}));
  await writeFile(path.join(dirty, 'Cargo.lock'), 'dirty root lock\n');
  await assert.rejects(
    () => provisionOpenApiCargoLock({repoRoot: dirty}),
    /working file must exactly match its HEAD blob/i,
  );

  const staged = await makeRepository();
  context.after(() => rm(staged, {recursive: true, force: true}));
  await writeFile(path.join(staged, 'Cargo.lock'), 'staged root lock\n');
  await gitText(staged, ['add', '--', 'Cargo.lock']);
  await assert.rejects(
    () => provisionOpenApiCargoLock({repoRoot: staged}),
    /index and HEAD entries must reference the same blob/i,
  );

  if (process.platform !== 'win32') {
    const executable = await makeRepository();
    context.after(() => rm(executable, {recursive: true, force: true}));
    await chmod(path.join(executable, 'Cargo.lock'), 0o755);
    await gitText(executable, ['add', '--', 'Cargo.lock']);
    await assert.rejects(
      () => provisionOpenApiCargoLock({repoRoot: executable}),
      /stage-zero 100644 Git blob/i,
    );
  }
});

test('staged Cargo.lock pin substitution is rejected', async (context) => {
  const root = await makeRepository();
  context.after(() => rm(root, {recursive: true, force: true}));
  const repositoryPinPath = path.join(
    root,
    OPENAPI_CARGO_LOCK_PIN_PATH,
  );
  await writeFile(
    repositoryPinPath,
    pinBytes.toString('utf8').replace(
      pin.sha256Hex,
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
        sourcePath: fixturePath,
      }),
    /pin index and HEAD entries must reference the same blob/i,
  );
});

test('unstaged Cargo.lock pin substitution is rejected', async (context) => {
  const root = await makeRepository();
  context.after(() => rm(root, {recursive: true, force: true}));
  const repositoryPinPath = path.join(root, OPENAPI_CARGO_LOCK_PIN_PATH);
  await writeFile(
    repositoryPinPath,
    encodeOpenApiCargoLockPin(Buffer.from('different lock bytes\n')),
  );

  await assert.rejects(
    () =>
      provisionOpenApiCargoLock({
        repoRoot: root,
        sourcePath: fixturePath,
      }),
    /working file must exactly match its HEAD blob/i,
  );
});

test(
  'comparison sources reject aliases, hard links, and executable files',
  {skip: process.platform === 'win32'},
  async (context) => {
    const root = await makeRepository();
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

test('comparison mutation and tracked-target replacement fail closed', async (context) => {
  const mutationRoot = await makeRepository();
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
        beforeVerify: async () => {
          const mutated = Buffer.from(fixtureBytes);
          mutated[0] ^= 0xff;
          await writeFile(source, mutated);
        },
      }),
    /replaced or mutated after read/i,
  );
  assert.deepEqual(
    await readFile(path.join(mutationRoot, 'Cargo.lock')),
    fixtureBytes,
  );

  const replacementRoot = await makeRepository();
  context.after(() => rm(replacementRoot, {recursive: true, force: true}));
  await writeFile(source, fixtureBytes);
  await assert.rejects(
    () =>
      provisionOpenApiCargoLock({
        repoRoot: replacementRoot,
        sourcePath: source,
        beforeVerify: () =>
          writeFile(path.join(replacementRoot, 'Cargo.lock'), fixtureBytes),
      }),
    /replaced or mutated/i,
  );
});

test('fixture Git commands ignore inherited repository routing', async (context) => {
  const sentinel = await makeTempDirectory('openapi-lock-git-sentinel-');
  const fixture = await makeTempDirectory('openapi-lock-git-fixture-');
  context.after(() => rm(sentinel, {recursive: true, force: true}));
  context.after(() => rm(fixture, {recursive: true, force: true}));

  await writeFile(path.join(sentinel, 'sentinel.txt'), 'sentinel\n');
  await gitText(sentinel, ['init', '--quiet']);
  await gitText(sentinel, [
    'config',
    'user.email',
    'sentinel@example.invalid',
  ]);
  await gitText(sentinel, ['config', 'user.name', 'Sentinel']);
  await gitText(sentinel, ['add', '.']);
  await gitText(sentinel, ['commit', '--quiet', '-m', 'sentinel']);

  const sentinelGitDirectory = path.join(sentinel, '.git');
  const sentinelConfigPath = path.join(sentinelGitDirectory, 'config');
  const sentinelIndexPath = path.join(sentinelGitDirectory, 'index');
  const [sentinelHeadBefore, sentinelConfigBefore, sentinelIndexBefore] =
    await Promise.all([
      gitText(sentinel, ['rev-parse', 'HEAD']),
      readFile(sentinelConfigPath),
      readFile(sentinelIndexPath),
    ]);
  const inheritedEnvironment = {
    ...process.env,
    GIT_COMMON_DIR: sentinelGitDirectory,
    GIT_CONFIG: sentinelConfigPath,
    GIT_DIR: sentinelGitDirectory,
    GIT_INDEX_FILE: sentinelIndexPath,
    GIT_OBJECT_DIRECTORY: path.join(sentinelGitDirectory, 'objects'),
    GIT_WORK_TREE: sentinel,
  };

  await writeFile(path.join(fixture, 'fixture.txt'), 'fixture\n');
  await gitText(fixture, ['init', '--quiet'], {
    environment: inheritedEnvironment,
  });
  await gitText(
    fixture,
    ['config', 'user.email', 'fixture@example.invalid'],
    {environment: inheritedEnvironment},
  );
  await gitText(fixture, ['config', 'user.name', 'Fixture'], {
    environment: inheritedEnvironment,
  });
  await gitText(fixture, ['add', '.'], {
    environment: inheritedEnvironment,
  });
  await gitText(fixture, ['commit', '--quiet', '-m', 'fixture'], {
    environment: inheritedEnvironment,
  });

  const fixtureTopLevel = await gitText(
    fixture,
    ['rev-parse', '--show-toplevel'],
    {environment: inheritedEnvironment},
  );
  assert.equal(
    await realpath(fixtureTopLevel.trim()),
    await realpath(fixture),
  );
  assert.equal(
    await gitText(sentinel, ['rev-parse', 'HEAD']),
    sentinelHeadBefore,
  );
  assert.deepEqual(await readFile(sentinelConfigPath), sentinelConfigBefore);
  assert.deepEqual(await readFile(sentinelIndexPath), sentinelIndexBefore);
});

test('OpenAPI canonical workflow validates a pre-existing lock before release gates', async () => {
  const workflow = await readFile(
    path.join(repoRoot, '.github', 'workflows', 'openapi.yml'),
    'utf8',
  );
  assert.equal(
    Array.from(
      workflow.matchAll(
        /provision-openapi-cargo-lock\.mjs pin/g,
      ),
    ).length,
    1,
  );
  assert.equal(
    Array.from(
      workflow.matchAll(
        /--source="\$\{repo_root\}\/Cargo\.lock"/g,
      ),
    ).length,
    1,
  );
  assert.equal(
    Array.from(
      workflow.matchAll(
        /--check="\$\{repo_root\}\/release\/openapi-cargo-lock-v1\.txt"/g,
      ),
    ).length,
    1,
  );
  assert.doesNotMatch(
    workflow,
    /provision-openapi-cargo-lock\.mjs provision/,
  );
  assert.equal(
    Array.from(
      workflow.matchAll(
        /git status --porcelain=v1 --untracked-files=all/g,
      ),
    ).length,
    1,
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
  assert.notEqual(metadataStart, -1);
  assert.notEqual(canonicalStart, -1);
  assert.ok(metadataStart < canonicalStart);
  const metadata = workflow.slice(metadataStart, canonicalStart);
  const canonical = workflow.slice(canonicalStart);
  for (const rootLockDependency of [
    /Cargo\.lock/,
    /release\/openapi-cargo-lock-v1\.txt/,
    /provision-openapi-cargo-lock\.mjs/,
  ]) {
    assert.doesNotMatch(metadata, rootLockDependency);
  }
  for (const rustGenerationSurface of [
    /\bcargo(?:\s|\+)/i,
    /\brust(?:c|fmt|up)\b/i,
    /\bxtask\b/,
    /generate-lockfile/,
    /check_openapi_spec\.sh/,
    /run_openapi_generator\.sh/,
  ]) {
    assert.doesNotMatch(metadata, rustGenerationSurface);
  }
  assert.match(metadata, /verify-openapi-release-inputs\.mjs/);
  assert.match(metadata, /verify-openapi-versions\.mjs --allow-unsigned/);
  assert.match(metadata, /check-openapi-signatures\.mjs/);
  assert.match(canonical, /provision-openapi-cargo-lock\.mjs pin/);
  assert.match(canonical, /--source="\$\{repo_root\}\/Cargo\.lock"/);
  assert.match(
    canonical,
    /--check="\$\{repo_root\}\/release\/openapi-cargo-lock-v1\.txt"/,
  );
  assert.ok(
    canonical.indexOf('provision-openapi-cargo-lock.mjs pin') <
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

async function makeRepository({trackRootLock = true} = {}) {
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
      '**/Cargo.lock\n!/Cargo.lock\n',
    ),
  ]);
  if (trackRootLock) {
    await writeFile(path.join(root, 'Cargo.lock'), fixtureBytes);
  }
  await gitText(root, ['init', '--quiet']);
  await gitText(root, ['config', 'user.email', 'openapi-test@example.invalid']);
  await gitText(root, ['config', 'user.name', 'OpenAPI Test']);
  await gitText(root, ['add', '.']);
  await gitText(root, ['commit', '--quiet', '-m', 'fixture']);
  return realpath(root);
}

async function gitText(
  root,
  arguments_,
  {environment = process.env} = {},
) {
  const {stdout} = await execFileAsync('git', arguments_, {
    cwd: root,
    encoding: 'utf8',
    env: isolateGitRepositoryEnvironment(environment),
  });
  return stdout;
}
