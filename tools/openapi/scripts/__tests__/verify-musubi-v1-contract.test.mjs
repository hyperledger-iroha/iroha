import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

import {MUSUBI_V1_MODELS, MUSUBI_V1_PATHS} from '../lib/musubi-v1-contract.mjs';
import {verifyCheckedMusubiV1Contracts} from '../verify-musubi-v1-contract.mjs';

function exactDocument() {
  return {
    openapi: '3.1.0',
    paths: Object.fromEntries(MUSUBI_V1_PATHS.map((path) => {
      const [requestType, responseType] = MUSUBI_V1_MODELS[path];
      return [path, {post: {
        tags: ['Musubi'],
        'x-iroha-norito-request-type': requestType,
        'x-iroha-norito-response-type': responseType,
        'x-iroha-tool-effect': path.startsWith('/v1/musubi/queries/')
          ? 'read'
          : 'build_instruction',
      }}];
    })),
  };
}

test('checked Musubi verifier reads both mutable OpenAPI aliases', async () => {
  const root = await mkdtemp(join(tmpdir(), 'musubi-openapi-contract-'));
  try {
    await mkdir(join(root, 'versions', 'current'), {recursive: true});
    const document = `${JSON.stringify(exactDocument())}\n`;
    await writeFile(join(root, 'torii.json'), document, {mode: 0o600});
    await writeFile(join(root, 'versions', 'current', 'torii.json'), document, {mode: 0o600});
    await assert.doesNotReject(() => verifyCheckedMusubiV1Contracts(root));

    const stale = exactDocument();
    delete stale.paths[MUSUBI_V1_PATHS[0]];
    await writeFile(
      join(root, 'versions', 'current', 'torii.json'),
      `${JSON.stringify(stale)}\n`,
      {mode: 0o600},
    );
    await assert.rejects(
      () => verifyCheckedMusubiV1Contracts(root),
      /stale Musubi route inventory/,
    );
  } finally {
    await rm(root, {recursive: true, force: true});
  }
});
