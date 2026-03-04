import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {execFile} from 'node:child_process';
import {mkdtemp, mkdir, readFile, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import {promisify} from 'node:util';

const execFileAsync = promisify(execFile);
const repoRoot = fileURLToPath(new URL('../../../../', import.meta.url));
const scriptPath = join(repoRoot, 'docs/portal/scripts/preview_verify.sh');
const writeChecksumsPath = join(repoRoot, 'docs/portal/scripts/write-checksums.mjs');

async function createBuildFixture() {
  const tmp = await mkdtemp(join(tmpdir(), 'preview-verify-'));
  const buildDir = join(tmp, 'build');
  await mkdir(join(buildDir, 'static'), {recursive: true});
  await writeFile(join(buildDir, 'index.html'), '<html>preview</html>\n', 'utf8');
  await writeFile(join(buildDir, 'static', 'main.js'), 'console.log("ok");\n', 'utf8');
  await execFileAsync('node', [writeChecksumsPath], {
    cwd: tmp,
    env: {...process.env, DOCS_RELEASE_SOURCE: 'test'},
  });
  return {tmp, buildDir};
}

async function baseDescriptor(manifestPath) {
  const manifestBytes = await readFile(manifestPath);
  const manifestSha = createHash('sha256').update(manifestBytes).digest('hex');
  return {
    version: 1,
    generated_at: new Date().toISOString(),
    checksums_manifest: {
      path: manifestPath,
      filename: 'checksums.sha256',
      sha256: manifestSha,
    },
  };
}

test('preview verification succeeds with descriptor + archive', async () => {
  const {tmp, buildDir} = await createBuildFixture();
  try {
    const manifestPath = join(buildDir, 'checksums.sha256');
    const descriptor = await baseDescriptor(manifestPath);
    const archivePath = join(tmp, 'preview-site.tar.gz');
    await writeFile(archivePath, 'pretend-archive\n', 'utf8');
    const archiveSha = createHash('sha256')
      .update(await readFile(archivePath))
      .digest('hex');
    descriptor.archive = {
      path: archivePath,
      filename: 'preview-site.tar.gz',
      sha256: archiveSha,
    };
    const descriptorPath = join(tmp, 'preview-descriptor.json');
    await writeFile(descriptorPath, JSON.stringify(descriptor, null, 2), 'utf8');

    const {stderr} = await execFileAsync(
      scriptPath,
      ['--build-dir', buildDir, '--descriptor', descriptorPath, '--archive', archivePath],
      {cwd: repoRoot},
    );
    assert.match(stderr, /preview verification complete/);
  } finally {
    await rm(tmp, {recursive: true, force: true});
  }
});

test('preview verification fails when descriptor hash mismatches', async () => {
  const {tmp, buildDir} = await createBuildFixture();
  try {
    const manifestPath = join(buildDir, 'checksums.sha256');
    const descriptor = await baseDescriptor(manifestPath);
    descriptor.checksums_manifest.sha256 = 'deadbeef';
    const descriptorPath = join(tmp, 'preview-descriptor.json');
    await writeFile(descriptorPath, JSON.stringify(descriptor, null, 2), 'utf8');

    await assert.rejects(
      execFileAsync(scriptPath, ['--build-dir', buildDir, '--descriptor', descriptorPath], {
        cwd: repoRoot,
      }),
      (error) =>
        typeof error.stderr === 'string' &&
        error.stderr.includes('descriptor manifest digest mismatch'),
      'expected manifest digest mismatch failure',
    );
  } finally {
    await rm(tmp, {recursive: true, force: true});
  }
});
