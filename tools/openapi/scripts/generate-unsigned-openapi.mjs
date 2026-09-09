#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0
/**
 * Generate unsigned V2 metadata for the clean, authored OpenAPI authority.
 * Prerequisites: Node >=24, installed tooling dependencies, exact clean HEAD,
 * and an existing empty owner-private output directory outside the checkout.
 * No Cargo, router execution, source writes or private signing inputs.
 */
import {createHash} from 'node:crypto';
import {lstat, mkdir, mkdtemp, open, readdir, realpath, rename, rm} from 'node:fs/promises';
import {dirname, isAbsolute, join, relative, resolve, sep} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';

import {computeOpenApiBlake3Hex} from './lib/openapi-manifest-v2.mjs';
import {validateReleaseOpenApiDocumentBytes} from './lib/openapi-provenance.mjs';
import {writeOpenApiAtomicFile} from './lib/openapi-safe-file.mjs';
import {buildVersionIndex, parseOpenApiVersionIndex} from './sync-openapi.mjs';
import {checkOpenApiSignatures} from './check-openapi-signatures.mjs';
import {
  captureOpenApiGeneratorSource,
  verifyOpenApiReleaseInputs,
} from './verify-openapi-release-inputs.mjs';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const GENERATOR_PATH = 'tools/openapi/scripts/generate-unsigned-openapi.mjs';
const SPEC_PATHS = Object.freeze([
  'artifacts/openapi/torii.json',
  'crates/iroha_torii/assets/openapi/torii.json',
  'artifacts/openapi/versions/current/torii.json',
]);
const SPEC_MAX_BYTES = 64 * 1024 * 1024;
const METADATA_MAX_BYTES = 1024 * 1024;
const json = (value) => `${JSON.stringify(value, null, 2)}\n`;
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

/** Publish one complete result by renaming a staged tree over an empty directory. */
export async function generateUnsignedOpenApi({
  sourceRoot = repoRoot, expectedCommit, outputDir, beforeFinalStateCheck,
}) {
  if (beforeFinalStateCheck !== undefined && typeof beforeFinalStateCheck !== 'function') {
    throw new TypeError('beforeFinalStateCheck must be a function');
  }
  const root = resolve(sourceRoot);
  const destination = await inspectDestination(root, outputDir);
  const source = await captureOpenApiGeneratorSource({repoRoot: root, expectedCommit});
  // The recursive `tools` entry in the source-input registry binds this owner.
  await source.readTrackedFile(GENERATOR_PATH, METADATA_MAX_BYTES);
  const specs = await Promise.all(SPEC_PATHS.map((path) => source.readTrackedFile(path, SPEC_MAX_BYTES)));
  if (!specs.every((bytes) => bytes.equals(specs[0]))) {
    throw new Error('authored, package-local and current OpenAPI specifications must be byte-identical');
  }
  const spec = specs[0];
  validateReleaseOpenApiDocumentBytes(spec, {label: 'authored Torii OpenAPI authority'});
  const manifest = {
    version: 2,
    generated_unix_ms: source.generatedUnixMs,
    generator_commit: source.commit,
    generator_dirty: false,
    generator_source_sha256_hex: source.sourceSha256Hex,
    artifact: {
      path: 'torii.json', bytes: spec.length, sha256_hex: sha256(spec),
      blake3_hex: computeOpenApiBlake3Hex(spec), signature: null,
    },
  };
  const stage = await mkdtemp(join(dirname(destination.path), '.openapi-authored-'));
  let published = false;
  try {
    const put = async (path, bytes) => {
      const target = join(stage, path);
      await mkdir(dirname(target), {recursive: true, mode: 0o700});
      await writeOpenApiAtomicFile(target, bytes, {label: `staged OpenAPI ${path}`});
    };
    const allowlist = await source.readTrackedFile('artifacts/openapi/allowed_signers.json', METADATA_MAX_BYTES);
    await put('allowed_signers.json', allowlist);
    // Retain only the two declared public files in each historical version.
    // Their existing manifest and signer policy remain authoritative.
    for (const label of await source.listTrackedDirectories('artifacts/openapi/versions')) {
      if (label === 'current') continue;
      for (const name of ['torii.json', 'manifest.json']) {
        const path = `versions/${label}/${name}`;
        await put(path, await source.readTrackedFile(`artifacts/openapi/${path}`, SPEC_MAX_BYTES));
      }
    }
    const previousIndexBytes = await source.readTrackedFile(
      'artifacts/openapi/versions.json', METADATA_MAX_BYTES, {optional: true},
    );
    const previousIndex = previousIndexBytes === null ? null : parseOpenApiVersionIndex(previousIndexBytes);
    for (const prefix of ['', 'versions/current/']) {
      await put(`${prefix}torii.json`, spec);
      await put(`${prefix}manifest.json`, json(manifest));
    }
    const index = await buildVersionIndex(
      join(stage, 'versions'), stage, join(stage, 'torii.json'), join(stage, 'manifest.json'),
      {previousIndex, generatedUnixMs: source.generatedUnixMs, allowedSignersFile: join(stage, 'allowed_signers.json')},
    );
    await put('versions.json', json(index));
    if (beforeFinalStateCheck) await beforeFinalStateCheck({sourceRoot: root, stage});
    // Validate the real V2 release-input contract before the one publication.
    const verification = await verifyOpenApiReleaseInputs({repoRoot: root, outputDir: stage});
    await checkOpenApiSignatures({
      staticDir: stage, versionsFile: join(stage, 'versions.json'),
      allowedSignersFile: join(stage, 'allowed_signers.json'),
      allowUnsigned: index.entries.filter((entry) => !entry.signed).map((entry) => entry.label),
    });
    const receipt = {
      ...verification,
      schema: 'iroha.openapi.unsigned_authored_spec.v1',
      generator: GENERATOR_PATH,
      authority: SPEC_PATHS[0],
      runtime_projection: 'not_executed',
      candidate_commit: source.commit,
      candidate_tree: source.tree,
    };
    // The receipt is separate from the closed V2 manifest schema.
    await put('unsigned-authored-spec.json', json(receipt));
    await source.assertUnchanged();
    const finalDestination = await inspectDestination(root, destination.path);
    if (finalDestination.dev !== destination.dev || finalDestination.ino !== destination.ino ||
        finalDestination.parentDev !== destination.parentDev || finalDestination.parentIno !== destination.parentIno) {
      throw new Error('OpenAPI output directory identity changed before publication');
    }
    // Both directories have the same private parent and the destination is empty.
    // Readers see either that empty directory or the entire validated result.
    await rename(stage, destination.path);
    published = true;
    const parent = await open(dirname(destination.path), 'r');
    try { await parent.sync(); } finally { await parent.close(); }
    return receipt;
  } finally {
    if (!published) await rm(stage, {recursive: true, force: true});
  }
}

async function inspectDestination(root, outputDir) {
  if (typeof outputDir !== 'string' || !isAbsolute(outputDir) || resolve(outputDir) !== outputDir) {
    throw new Error('OpenAPI output must be an absolute canonical directory');
  }
  const source = await realpath(root);
  const within = relative(source, outputDir);
  if (within === '' || (within !== '..' && !within.startsWith(`..${sep}`) && !isAbsolute(within))) {
    throw new Error('OpenAPI output must remain outside the source checkout');
  }
  const parentPath = dirname(outputDir);
  if (await realpath(outputDir) !== outputDir || await realpath(parentPath) !== parentPath) {
    throw new Error('OpenAPI output and parent must not contain symlinks');
  }
  const [output, parent] = await Promise.all([lstat(outputDir), lstat(parentPath)]);
  for (const state of [output, parent]) {
    if (!state.isDirectory() || state.isSymbolicLink() || state.uid !== process.getuid() || (state.mode & 0o077) !== 0) {
      throw new Error('OpenAPI output and parent must be owner-private directories');
    }
  }
  if ((await readdir(outputDir)).length !== 0) {
    throw new Error('OpenAPI output directory must be empty; existing contents are preserved');
  }
  return {path: outputDir, dev: output.dev, ino: output.ino, parentDev: parent.dev, parentIno: parent.ino};
}

async function runCli(argv) {
  if (argv.length === 1 && argv[0] === '--help') {
    console.log('usage: generate-unsigned-openapi.mjs --source-commit=<full-clean-HEAD> --output-dir=<absolute-empty-private-directory>');
    return;
  }
  const options = {};
  for (const arg of argv) {
    const match = /^--(source-commit|output-dir)=(.+)$/.exec(arg);
    if (!match || Object.hasOwn(options, match[1])) throw new Error('expected one --source-commit and one --output-dir');
    options[match[1]] = match[2];
  }
  console.log(JSON.stringify(await generateUnsignedOpenApi({
    expectedCommit: options['source-commit'], outputDir: options['output-dir'],
  })));
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  runCli(process.argv.slice(2)).catch((error) => { console.error(error.message); process.exitCode = 1; });
}
