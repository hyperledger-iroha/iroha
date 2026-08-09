#!/usr/bin/env node
/** Fail closed unless checked latest/current specs expose the exact Musubi V1 contract. */
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';

import {scanJsonRejectDuplicateKeys} from './lib/openapi-manifest-v2.mjs';
import {readOpenApiStableFile} from './lib/openapi-safe-file.mjs';
import {verifyMusubiV1OpenApiContract} from './lib/musubi-v1-contract.mjs';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const defaultOutputDir = resolve(scriptDir, '..', '..', '..', 'artifacts', 'openapi');
const SPEC_MAX_BYTES = 64 * 1024 * 1024;

export async function verifyCheckedMusubiV1Contracts(outputDir = defaultOutputDir) {
  for (const relativePath of ['torii.json', join('versions', 'current', 'torii.json')]) {
    const path = join(outputDir, relativePath);
    const text = await readOpenApiStableFile(path, {
      label: `checked OpenAPI specification ${path}`,
      maxBytes: SPEC_MAX_BYTES,
      encoding: 'utf8',
    });
    scanJsonRejectDuplicateKeys(text, `checked OpenAPI specification ${path}`);
    let document;
    try {
      document = JSON.parse(text);
    } catch (error) {
      throw new Error(`failed to parse checked OpenAPI specification ${path}: ${error?.message ?? error}`);
    }
    verifyMusubiV1OpenApiContract(document, relativePath.split('\\').join('/'));
  }
}

const invokedUrl = process.argv[1] ? pathToFileURL(process.argv[1]).href : undefined;
if (invokedUrl === import.meta.url) {
  if (process.argv.length !== 2) {
    console.error('verify-musubi-v1-contract accepts no command-line arguments');
    process.exit(2);
  }
  verifyCheckedMusubiV1Contracts().catch((error) => {
    console.error(error.message ?? error);
    process.exit(1);
  });
}
