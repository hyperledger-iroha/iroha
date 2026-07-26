import {test} from 'node:test';
import assert from 'node:assert/strict';
import {
  chmod,
  link,
  mkdir,
  mkdtemp,
  readFile,
  stat,
  symlink,
  writeFile,
} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

import {
  readOpenApiStableFile,
  writeOpenApiAtomicFile,
} from '../lib/openapi-safe-file.mjs';

test('readOpenApiStableFile reads bounded UTF-8 from a regular file', async () => {
  const root = await mkdtemp(join(tmpdir(), 'openapi-safe-file-'));
  const input = join(root, 'input.json');
  await writeFile(input, '{"ok":true}\n');

  assert.equal(
    await readOpenApiStableFile(input, {
      label: 'fixture',
      maxBytes: 64,
      encoding: 'utf8',
    }),
    '{"ok":true}\n',
  );
});

test('readOpenApiStableFile rejects oversized and malformed UTF-8 input', async () => {
  const root = await mkdtemp(join(tmpdir(), 'openapi-safe-file-bounds-'));
  const oversized = join(root, 'oversized.bin');
  await writeFile(oversized, Buffer.alloc(17));
  await assert.rejects(
    () =>
      readOpenApiStableFile(oversized, {
        label: 'oversized fixture',
        maxBytes: 16,
      }),
    /exceeds the 16-byte limit/i,
  );

  const malformed = join(root, 'malformed.json');
  await writeFile(malformed, Buffer.from([0xff]));
  await assert.rejects(
    () =>
      readOpenApiStableFile(malformed, {
        label: 'malformed fixture',
        maxBytes: 16,
        encoding: 'utf8',
      }),
    /must be UTF-8/i,
  );
});

test(
  'readOpenApiStableFile rejects symlinks, hardlinks, and symlinked parents',
  {skip: process.platform === 'win32'},
  async () => {
    const root = await mkdtemp(join(tmpdir(), 'openapi-safe-file-links-'));
    const protectedPath = join(root, 'protected.json');
    await writeFile(protectedPath, '{}');

    const symlinkPath = join(root, 'symlink.json');
    await symlink(protectedPath, symlinkPath);
    await assert.rejects(
      () =>
        readOpenApiStableFile(symlinkPath, {
          label: 'symlink fixture',
          maxBytes: 16,
        }),
      /symlink/i,
    );

    const hardlinkPath = join(root, 'hardlink.json');
    await link(protectedPath, hardlinkPath);
    await assert.rejects(
      () =>
        readOpenApiStableFile(hardlinkPath, {
          label: 'hardlink fixture',
          maxBytes: 16,
        }),
      /hard link/i,
    );

    const realParent = join(root, 'real-parent');
    await mkdir(realParent);
    const nested = join(realParent, 'input.json');
    await writeFile(nested, '{}');
    const aliasParent = join(root, 'alias-parent');
    await symlink(realParent, aliasParent);
    await assert.rejects(
      () =>
        readOpenApiStableFile(join(aliasParent, 'input.json'), {
          label: 'parent fixture',
          maxBytes: 16,
        }),
      /parent .* must not be a symlink/i,
    );
  },
);

test(
  'readOpenApiStableFile rejects writable trust roots',
  {skip: process.platform === 'win32'},
  async () => {
    const root = await mkdtemp(join(tmpdir(), 'openapi-safe-file-mode-'));
    const input = join(root, 'trust.json');
    await writeFile(input, '{}');
    await chmod(input, 0o666);
    await assert.rejects(
      () =>
        readOpenApiStableFile(input, {
          label: 'trust fixture',
          maxBytes: 16,
          requireSafePermissions: true,
        }),
      /must not be writable by group or other users/i,
    );
  },
);

test('writeOpenApiAtomicFile creates and replaces exact public bytes', async () => {
  const root = await mkdtemp(join(tmpdir(), 'openapi-atomic-file-'));
  const output = join(root, 'manifest.json');
  await writeOpenApiAtomicFile(output, '{"version":1}\n', {
    label: 'fixture output',
  });
  assert.equal(await readFile(output, 'utf8'), '{"version":1}\n');

  await writeOpenApiAtomicFile(output, Buffer.from('{"version":2}\n'), {
    label: 'fixture output',
  });
  assert.equal(await readFile(output, 'utf8'), '{"version":2}\n');
  if (process.platform !== 'win32') {
    const metadata = await stat(output);
    assert.equal(metadata.mode & 0o777, 0o644);
  }
});

test(
  'writeOpenApiAtomicFile refuses linked destinations',
  {skip: process.platform === 'win32'},
  async () => {
    for (const targetKind of ['symlink', 'hardlink']) {
      const root = await mkdtemp(
        join(tmpdir(), `openapi-atomic-${targetKind}-`),
      );
      const protectedPath = join(root, 'protected.json');
      const output = join(root, 'manifest.json');
      await writeFile(protectedPath, 'protected');
      if (targetKind === 'symlink') {
        await symlink(protectedPath, output);
      } else {
        await link(protectedPath, output);
      }
      await assert.rejects(
        () =>
          writeOpenApiAtomicFile(output, 'replacement', {
            label: 'linked fixture',
          }),
        /symlink|hard link/i,
      );
      assert.equal(await readFile(protectedPath, 'utf8'), 'protected');
    }
  },
);
