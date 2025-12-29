#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0

import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';

import {verifyOpenApiSignature} from './lib/openapi-signature.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const defaultStaticDir = path.join(__dirname, '..', 'static', 'openapi');
const defaultVersionsFile = path.join(defaultStaticDir, 'versions.json');
const defaultAllowedSignersFile = path.join(defaultStaticDir, 'allowed_signers.json');

export function parseArgs(argv) {
  const options = {
    allowUnsigned: [],
  };
  for (const arg of argv) {
    if (arg === '--json') {
      options.json = true;
      continue;
    }
    if (arg.startsWith('--allow-unsigned=')) {
      const label = arg.slice('--allow-unsigned='.length).trim();
      if (label) {
        options.allowUnsigned.push(label);
      }
      continue;
    }
    if (arg.startsWith('--static-dir=')) {
      const value = arg.slice('--static-dir='.length).trim();
      if (value) {
        options.staticDir = path.resolve(value);
      }
      continue;
    }
    if (arg.startsWith('--versions=')) {
      const value = arg.slice('--versions='.length).trim();
      if (value) {
        options.versionsFile = path.resolve(value);
      }
      continue;
    }
    if (arg.startsWith('--allowed-signers=')) {
      const value = arg.slice('--allowed-signers='.length).trim();
      if (value) {
        options.allowedSignersFile = path.resolve(value);
      }
      continue;
    }
  }
  return options;
}

const defaultOptions = {
  staticDir: defaultStaticDir,
  versionsFile: defaultVersionsFile,
  allowedSignersFile: defaultAllowedSignersFile,
  allowUnsigned: [],
  json: false,
};

function normalizeOptions(options) {
  const allowUnsigned = Array.from(
    new Set((options.allowUnsigned ?? []).map((entry) => entry.trim()).filter(Boolean)),
  );
  const staticDir = options.staticDir ?? defaultStaticDir;
  const versionsFile = options.versionsFile ?? path.join(staticDir, 'versions.json');
  const allowedSignersFile =
    options.allowedSignersFile ?? path.join(staticDir, 'allowed_signers.json');
  return {
    ...defaultOptions,
    ...options,
    staticDir,
    versionsFile,
    allowedSignersFile,
    allowUnsigned,
  };
}

export async function checkOpenApiSignatures(options = {}) {
  const {
    staticDir,
    versionsFile,
    allowedSignersFile,
    allowUnsigned,
  } = normalizeOptions(options);

  const summary = {
    staticDir,
    versionsFile,
    allowedSignersFile,
    checkedLabels: [],
    skippedLabels: [],
    allowedSignerCount: 0,
    issues: [],
  };

  const allowedSigners = await loadAllowedSigners(allowedSignersFile);
  summary.allowedSignerCount = allowedSigners.size;

  const manifestRaw = await readFile(versionsFile, 'utf8');
  let manifest;
  try {
    manifest = JSON.parse(manifestRaw);
  } catch (error) {
    throw new Error(`failed to parse ${versionsFile}: ${error.message}`);
  }

  if (!manifest || !Array.isArray(manifest.entries)) {
    throw new Error(`${versionsFile} is missing an entries array`);
  }

  const declaredVersions = normalizeVersions(manifest, versionsFile);
  const entryLabels = new Set();

  for (const entry of manifest.entries) {
    const label = typeof entry?.label === 'string' ? entry.label : null;
    const displayLabel = label ?? '(unknown)';
    const entryIssues = [];
    if (label) {
      if (entryLabels.has(label)) {
        entryIssues.push(`duplicate label '${label}' in versions manifest`);
      } else {
        entryLabels.add(label);
      }
    }
    if (allowUnsigned.includes(label)) {
      if (entryIssues.length > 0) {
        summary.issues.push({
          label: displayLabel,
          errors: entryIssues,
        });
      } else {
        summary.skippedLabels.push(displayLabel);
      }
      continue;
    }
    if (!entry || typeof entry !== 'object') {
      summary.issues.push({
        label: displayLabel,
        errors: ['entry is not an object'],
      });
      continue;
    }
    if (!entry.signed) {
      entryIssues.push('entry is not flagged as signed');
    }
    const entryBytes = typeof entry.bytes === 'number' ? entry.bytes : null;
    if (entryBytes === null) {
      entryIssues.push('versions entry missing bytes');
    }
    const entrySignature = {
      algorithm: normalizeAlgorithm(entry.signatureAlgorithm),
      publicKey: normalizeHex(entry.signaturePublicKeyHex),
      signature: normalizeHex(entry.signatureHex),
    };
    if (!entrySignature.algorithm) {
      entryIssues.push('versions entry missing signatureAlgorithm');
    }
    if (!entrySignature.publicKey) {
      entryIssues.push('versions entry missing signaturePublicKeyHex');
    }
    if (!entrySignature.signature) {
      entryIssues.push('versions entry missing signatureHex');
    }
    if (
      entrySignature.algorithm &&
      entrySignature.publicKey &&
      !isAllowedSigner(allowedSigners, entrySignature)
    ) {
      entryIssues.push('versions entry signer not allowed');
    }
    const specPath = normalizeRelative(entry.path);
    if (!specPath) {
      entryIssues.push('missing spec path');
    }
    let specBuffer = null;
    let computedSha256 = null;
    let specByteLength = null;
    const specFullPath = specPath ? path.join(staticDir, specPath) : null;
    if (specFullPath) {
      try {
        specBuffer = await readFile(specFullPath);
        computedSha256 = computeSha256Hex(specBuffer);
        specByteLength = specBuffer.byteLength;
        const expectedSha = normalizeHex(entry.sha256);
        if (!expectedSha) {
          entryIssues.push('missing sha256 in versions manifest');
        } else if (expectedSha !== computedSha256) {
          entryIssues.push(
            `sha256 mismatch (manifest: ${expectedSha}, computed: ${computedSha256})`,
          );
        }
        if (entryBytes !== null && entryBytes !== specByteLength) {
          entryIssues.push(
            `bytes mismatch (versions entry: ${entryBytes}, file: ${specByteLength})`,
          );
        }
      } catch (error) {
        entryIssues.push(`failed to read spec ${specPath}: ${error.message ?? error}`);
      }
    }

    const manifestPath = normalizeRelative(entry.manifestPath);
    if (!manifestPath) {
      entryIssues.push('missing manifest path');
    }

    let manifestJson = null;
    const manifestFullPath = manifestPath ? path.join(staticDir, manifestPath) : null;
    if (manifestFullPath) {
      try {
        const data = await readFile(manifestFullPath, 'utf8');
        manifestJson = JSON.parse(data);
      } catch (error) {
        entryIssues.push(
          `failed to load manifest ${manifestPath}: ${error.message ?? error}`,
        );
      }
    }

    if (manifestJson) {
      const artifact = manifestJson.artifact;
      if (!artifact) {
        entryIssues.push('manifest missing artifact metadata');
      } else {
        const manifestBytes =
          typeof artifact.bytes === 'number' ? artifact.bytes : null;
        if (manifestBytes === null) {
          entryIssues.push('manifest missing artifact.bytes');
        } else {
          if (specByteLength !== null && manifestBytes !== specByteLength) {
            entryIssues.push(
              `manifest bytes mismatch (manifest: ${manifestBytes}, file: ${specByteLength})`,
            );
          }
          if (entryBytes !== null && manifestBytes !== entryBytes) {
            entryIssues.push(
              `manifest bytes mismatch (manifest: ${manifestBytes}, versions: ${entryBytes})`,
            );
          }
        }
        if (specPath && normalizePath(artifact.path) !== normalizePath(specPath)) {
          entryIssues.push(
            `manifest references ${artifact.path ?? '(missing)'}, expected ${specPath}`,
          );
        }
        const manifestSha = normalizeHex(artifact.sha256_hex);
        if (!manifestSha) {
          entryIssues.push('manifest missing artifact.sha256_hex');
        } else if (computedSha256 && manifestSha !== computedSha256) {
          entryIssues.push(
            `manifest sha256 mismatch (manifest: ${manifestSha}, computed: ${computedSha256})`,
          );
        }
        if (entry.blake3) {
          const recorded = normalizeHex(entry.blake3);
          const manifestBlake3 = normalizeHex(artifact.blake3_hex);
          if (!manifestBlake3) {
            entryIssues.push('manifest missing artifact.blake3_hex');
          } else if (recorded && manifestBlake3 !== recorded) {
            entryIssues.push('manifest blake3 mismatch');
          }
        }
        const signature = artifact.signature;
        if (!signature) {
          entryIssues.push('manifest missing artifact.signature');
        } else {
          const manifestSignatureAlgorithm = normalizeAlgorithm(signature.algorithm);
          if (!manifestSignatureAlgorithm) {
            entryIssues.push('signature missing algorithm');
          }
          const manifestSignaturePublicKey = normalizeHex(signature.public_key_hex);
          if (!manifestSignaturePublicKey) {
            entryIssues.push('signature missing public key');
          }
          const manifestSignatureValue = normalizeHex(signature.signature_hex);
          if (!manifestSignatureValue) {
            entryIssues.push('signature missing value');
          }
          if (
            manifestSignatureAlgorithm &&
            entrySignature.algorithm &&
            manifestSignatureAlgorithm !== entrySignature.algorithm
          ) {
            entryIssues.push(
              `signature algorithm mismatch (versions: ${entrySignature.algorithm}, manifest: ${manifestSignatureAlgorithm})`,
            );
          }
          if (
            manifestSignaturePublicKey &&
            entrySignature.publicKey &&
            manifestSignaturePublicKey !== entrySignature.publicKey
          ) {
            entryIssues.push('signature public key mismatch between versions entry and manifest');
          }
          if (
            manifestSignatureAlgorithm &&
            manifestSignaturePublicKey &&
            !isAllowedSigner(allowedSigners, {
              algorithm: manifestSignatureAlgorithm,
              publicKey: manifestSignaturePublicKey,
            })
          ) {
            entryIssues.push('manifest signer not allowed');
          }
          if (
            manifestSignatureValue &&
            entrySignature.signature &&
            manifestSignatureValue !== entrySignature.signature
          ) {
            entryIssues.push('signature value mismatch between versions entry and manifest');
          }
          if (
            manifestSignatureAlgorithm &&
            manifestSignaturePublicKey &&
            manifestSignatureValue &&
            specBuffer
          ) {
            try {
              verifyOpenApiSignature({
                algorithm: manifestSignatureAlgorithm,
                publicKeyHex: manifestSignaturePublicKey,
                signatureHex: manifestSignatureValue,
                payload: specBuffer,
              });
            } catch (error) {
              entryIssues.push(
                `signature verification failed: ${error.message ?? error}`,
              );
            }
          }
        }
      }
    }

    if (entryIssues.length > 0) {
      summary.issues.push({
        label: displayLabel,
        errors: entryIssues,
      });
    } else {
      summary.checkedLabels.push(displayLabel);
    }
  }

  const missingVersions = declaredVersions.filter((label) => !entryLabels.has(label));
  if (missingVersions.length > 0) {
    summary.issues.push({
      label: versionsFile,
      errors: [
        `versions list does not have matching entries for: ${missingVersions.join(', ')}`,
      ],
    });
  }

  if (summary.issues.length > 0) {
    throw new Error(formatIssues(summary.issues));
  }

  return summary;
}

function formatIssues(issueList) {
  const lines = issueList.map(
    (issue) => `- ${issue.label}: ${issue.errors.join('; ')}`,
  );
  return `OpenAPI signature verification failed:\n${lines.join('\n')}`;
}

function computeSha256Hex(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

function normalizeHex(value) {
  return typeof value === 'string' ? value.toLowerCase() : null;
}

function normalizeAlgorithm(value) {
  return typeof value === 'string' && value.trim() !== ''
    ? value.trim().toLowerCase()
    : null;
}

function normalizeVersions(manifest, versionsFile) {
  if (
    !manifest ||
    manifest.versions === undefined ||
    manifest.versions === null
  ) {
    throw new Error(`${versionsFile} is missing a versions array`);
  }
  if (!Array.isArray(manifest.versions)) {
    throw new Error(`${versionsFile} versions must be an array`);
  }
  const issues = [];
  const labels = [];
  manifest.versions.forEach((value, index) => {
    if (typeof value !== 'string' || value.trim() === '') {
      issues.push(`versions[${index}] must be a non-empty string`);
      return;
    }
    labels.push(value);
  });
  if (issues.length > 0) {
    throw new Error(
      `invalid versions list in ${versionsFile}:\n${issues
        .map((issue) => `- ${issue}`)
        .join('\n')}`,
    );
  }
  return labels;
}

async function loadAllowedSigners(allowedSignersFile) {
  let raw = null;
  try {
    raw = await readFile(allowedSignersFile, 'utf8');
  } catch (error) {
    throw new Error(
      `failed to read allowed signers file ${allowedSignersFile}: ${error.message ?? error}`,
    );
  }
  let parsed = null;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    throw new Error(`failed to parse ${allowedSignersFile}: ${error.message ?? error}`);
  }
  if (!parsed || !Array.isArray(parsed.allow)) {
    throw new Error(`${allowedSignersFile} must contain an allow array`);
  }
  const entries = [];
  const issues = [];
  for (const [index, entry] of parsed.allow.entries()) {
    const algorithm = normalizeAlgorithm(entry?.algorithm);
    const publicKey = normalizeHex(entry?.public_key_hex ?? entry?.publicKeyHex);
    if (!algorithm) {
      issues.push(`entry ${index} missing algorithm`);
    }
    if (!publicKey) {
      issues.push(`entry ${index} missing public_key_hex`);
    }
    if (algorithm && publicKey) {
      entries.push(formatSignerKey(algorithm, publicKey));
    }
  }
  if (entries.length === 0) {
    issues.push('no allowed signers specified');
  }
  if (issues.length > 0) {
    throw new Error(
      `invalid ${allowedSignersFile}:\n${issues.map((issue) => `- ${issue}`).join('\n')}`,
    );
  }
  return new Set(entries);
}

function formatSignerKey(algorithm, publicKey) {
  return `${algorithm}:${publicKey}`;
}

function isAllowedSigner(allowedSigners, signature) {
  const algorithm = normalizeAlgorithm(signature.algorithm);
  const publicKey = normalizeHex(signature.publicKey);
  if (!algorithm || !publicKey) {
    return false;
  }
  return allowedSigners.has(formatSignerKey(algorithm, publicKey));
}

function normalizeRelative(value) {
  if (typeof value !== 'string' || value.trim() === '') {
    return null;
  }
  return value.replace(/^[/\\]+/, '').trim();
}

function normalizePath(value) {
  return typeof value === 'string' ? value.replace(/\\/g, '/') : value;
}

async function runCli() {
  const options = parseArgs(process.argv.slice(2));
  const summary = await checkOpenApiSignatures(options);
  if (options.json) {
    console.log(JSON.stringify(summary, null, 2));
  } else {
    console.log(
      `[openapi-signatures] verified ${summary.checkedLabels.length} entries ` +
        `(skipped ${summary.skippedLabels.length})`,
    );
  }
}

const invokedUrl = process.argv[1] ? pathToFileURL(process.argv[1]).href : undefined;
if (invokedUrl === import.meta.url) {
  runCli().catch((error) => {
    console.error(error.message ?? error);
    process.exit(1);
  });
}
