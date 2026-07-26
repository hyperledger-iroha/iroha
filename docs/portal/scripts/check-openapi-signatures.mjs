#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0

import {createHash} from 'node:crypto';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';

import {
  validateReleaseOpenApiDocumentBytes,
} from './lib/openapi-provenance.mjs';
import {
  OPENAPI_MANIFEST_VERSION,
  parseOpenApiManifestV2Json,
  scanJsonRejectDuplicateKeys,
  verifyOpenApiManifestV2,
} from './lib/openapi-manifest-v2.mjs';
import {readOpenApiStableFile} from './lib/openapi-safe-file.mjs';
import {isAllowedSigner, loadAllowedSigners} from './lib/openapi-signers.mjs';
import {
  rejectUnknownOpenApiVersionsIndexFields,
  requireOpenApiVersionsIndexFields,
} from './lib/openapi-versions-index.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const defaultStaticDir = path.join(__dirname, '..', 'static', 'openapi');
const defaultVersionsFile = path.join(defaultStaticDir, 'versions.json');
const defaultAllowedSignersFile = path.join(defaultStaticDir, 'allowed_signers.json');
const OPENAPI_SPEC_MAX_BYTES = 64 * 1024 * 1024;
const OPENAPI_MANIFEST_MAX_BYTES = 64 * 1024;
const OPENAPI_VERSIONS_MAX_BYTES = 1024 * 1024;

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

  const manifestRaw = await readOpenApiStableFile(versionsFile, {
    label: 'OpenAPI versions manifest',
    maxBytes: OPENAPI_VERSIONS_MAX_BYTES,
    encoding: 'utf8',
  });
  scanJsonRejectDuplicateKeys(manifestRaw, `versions manifest ${versionsFile}`);
  let manifest;
  try {
    manifest = JSON.parse(manifestRaw);
  } catch (error) {
    throw new Error(`failed to parse ${versionsFile}: ${error.message}`);
  }
  rejectUnknownOpenApiVersionsIndexFields(manifest, {
    label: `versions manifest ${versionsFile}`,
  });

  if (!manifest || !Array.isArray(manifest.entries)) {
    throw new Error(`${versionsFile} is missing an entries array`);
  }

  const declaredVersions = normalizeVersions(manifest, versionsFile);
  const entryLabels = new Set();

  for (const entry of manifest.entries) {
    const label = typeof entry?.label === 'string' ? entry.label : null;
    const displayLabel = label ?? '(unknown)';
    const unsignedAllowed = label ? allowUnsigned.includes(label) : false;
    const entryIssues = [];
    if (label) {
      if (entryLabels.has(label)) {
        entryIssues.push(`duplicate label '${label}' in versions manifest`);
      } else {
        entryLabels.add(label);
      }
    }
    if (!entry || typeof entry !== 'object') {
      summary.issues.push({
        label: displayLabel,
        errors: ['entry is not an object'],
      });
      continue;
    }
    if (!isNonEmptyString(label)) {
      entryIssues.push('versions entry missing label');
    } else if (label.trim() !== label) {
      entryIssues.push('versions entry label must not have surrounding whitespace');
    }
    if (typeof entry.signed !== 'boolean') {
      entryIssues.push('versions entry signed must be boolean');
    }
    const entrySigned = entry.signed === true;
    const requiresSignature = entrySigned || !unsignedAllowed;
    if (!entrySigned && requiresSignature) {
      entryIssues.push('entry is not flagged as signed');
    }
    const entryBytes =
      Number.isSafeInteger(entry.bytes) && entry.bytes >= 0 ? entry.bytes : null;
    if (entryBytes === null) {
      entryIssues.push('versions entry missing bytes');
    }
    const entrySignature = {
      algorithm: normalizeAlgorithm(entry.signatureAlgorithm),
      publicKey: normalizeHex(entry.signaturePublicKeyHex),
      signature: normalizeHex(entry.signatureHex),
    };
    if (entrySignature.algorithm && entrySignature.algorithm !== 'ed25519') {
      entryIssues.push(`versions entry unsupported signatureAlgorithm ${entrySignature.algorithm}`);
    }
    if (entrySignature.publicKey && !isEd25519PublicKeyHex(entrySignature.publicKey)) {
      entryIssues.push('versions entry invalid signaturePublicKeyHex');
    }
    if (entrySignature.signature && !isEd25519SignatureHex(entrySignature.signature)) {
      entryIssues.push('versions entry invalid signatureHex');
    }
    if (
      unsignedAllowed &&
      !entrySigned &&
      (entrySignature.algorithm || entrySignature.publicKey || entrySignature.signature)
    ) {
      entryIssues.push('unsigned versions entry must not include signature metadata');
    }
    if (requiresSignature && !entrySignature.algorithm) {
      entryIssues.push('versions entry missing signatureAlgorithm');
    }
    if (requiresSignature && !entrySignature.publicKey) {
      entryIssues.push('versions entry missing signaturePublicKeyHex');
    }
    if (requiresSignature && !entrySignature.signature) {
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
        specBuffer = await readOpenApiStableFile(specFullPath, {
          label: `OpenAPI spec ${specPath}`,
          maxBytes: OPENAPI_SPEC_MAX_BYTES,
        });
        validateReleaseOpenApiDocumentBytes(specBuffer, {
          label: `OpenAPI spec ${specPath}`,
        });
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
        entryIssues.push(
          `failed to read or validate spec ${specPath}: ${error.message ?? error}`,
        );
      }
    }

    const manifestPath = normalizeRelative(entry.manifestPath);
    if (!manifestPath && requiresSignature) {
      entryIssues.push('missing manifest path');
    }

    let manifestJson = null;
    const manifestFullPath = manifestPath ? path.join(staticDir, manifestPath) : null;
    if (manifestFullPath) {
      try {
        const data = await readOpenApiStableFile(manifestFullPath, {
          label: `OpenAPI manifest ${manifestPath}`,
          maxBytes: OPENAPI_MANIFEST_MAX_BYTES,
          encoding: 'utf8',
        });
        manifestJson = parseOpenApiManifestV2Json(data, {
          label: `manifest ${manifestPath}`,
        });
      } catch (error) {
        entryIssues.push(
          `failed to load manifest ${manifestPath}: ${error.message ?? error}`,
        );
      }
    }

    if (manifestJson) {
      if (manifestJson.version !== OPENAPI_MANIFEST_VERSION) {
        entryIssues.push('manifest unsupported version');
      }
      if (
        !Number.isSafeInteger(manifestJson.generated_unix_ms) ||
        manifestJson.generated_unix_ms < 0
      ) {
        entryIssues.push('manifest missing generated_unix_ms');
      }
      if (specBuffer && manifestFullPath && specFullPath) {
        try {
          verifyOpenApiManifestV2({
            manifest: manifestJson,
            artifactBytes: specBuffer,
            label: `manifest ${manifestPath}`,
            expectedArtifactPath: path
              .relative(path.dirname(manifestFullPath), specFullPath)
              .split(path.sep)
              .join('/'),
            requireSignature: requiresSignature,
            requireClean: requiresSignature,
          });
        } catch (error) {
          entryIssues.push(error.message ?? String(error));
        }
      }
      const artifact = manifestJson.artifact;
      if (!artifact || typeof artifact !== 'object' || Array.isArray(artifact)) {
        entryIssues.push('manifest missing artifact metadata');
      } else {
        const manifestBytes =
          Number.isSafeInteger(artifact.bytes) && artifact.bytes >= 0 ? artifact.bytes : null;
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
        const artifactPath = normalizeArtifactPath(artifact.path);
        if (!artifactPath) {
          entryIssues.push('manifest missing or invalid artifact.path');
        } else if (
          specFullPath &&
          manifestFullPath &&
          artifactPath !==
            path
              .relative(path.dirname(manifestFullPath), specFullPath)
              .split(path.sep)
              .join('/')
        ) {
          entryIssues.push(
            `manifest references ${artifact.path ?? '(missing)'}, expected a path relative to its containing manifest`,
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
        if (entry.blake3 !== undefined && entry.blake3 !== null) {
          const recorded = normalizeHex(entry.blake3);
          if (!isBlake3Hex(recorded)) {
            entryIssues.push('versions entry invalid blake3');
          }
          const manifestBlake3 = normalizeHex(artifact.blake3_hex);
          if (!manifestBlake3) {
            entryIssues.push('manifest missing artifact.blake3_hex');
          } else if (recorded && isBlake3Hex(recorded) && manifestBlake3 !== recorded) {
            entryIssues.push('manifest blake3 mismatch');
          }
        }
        const signature = artifact.signature;
        if (!signature) {
          if (requiresSignature) {
            entryIssues.push('manifest missing artifact.signature');
          }
        } else if (typeof signature !== 'object' || Array.isArray(signature)) {
          entryIssues.push('manifest signature must be an object');
        } else {
          if (!requiresSignature) {
            entryIssues.push('unsigned manifest must not include artifact.signature');
          }
          const manifestSignatureAlgorithm = normalizeAlgorithm(signature.algorithm);
          if (!manifestSignatureAlgorithm) {
            entryIssues.push('signature missing algorithm');
          }
          const manifestSignaturePublicKey = normalizeHex(signature.public_key_hex);
          if (!manifestSignaturePublicKey) {
            entryIssues.push('signature missing public key');
          } else if (!isEd25519PublicKeyHex(manifestSignaturePublicKey)) {
            entryIssues.push('signature invalid public key');
          }
          const manifestSignatureValue = normalizeHex(signature.signature_hex);
          if (!manifestSignatureValue) {
            entryIssues.push('signature missing value');
          } else if (!isEd25519SignatureHex(manifestSignatureValue)) {
            entryIssues.push('signature invalid value');
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
        }
      }
    }

    if (entryIssues.length > 0) {
      summary.issues.push({
        label: displayLabel,
        errors: entryIssues,
      });
    } else if (unsignedAllowed && !entry.signed) {
      summary.skippedLabels.push(displayLabel);
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
  requireOpenApiVersionsIndexFields(manifest, {
    label: `versions manifest ${versionsFile}`,
  });

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
  return typeof value === 'string' ? value : null;
}

function normalizeAlgorithm(value) {
  return typeof value === 'string' && value !== '' ? value : null;
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
  const seen = new Set();
  manifest.versions.forEach((value, index) => {
    if (typeof value !== 'string' || value.trim() === '') {
      issues.push(`versions[${index}] must be a non-empty string`);
      return;
    }
    if (value.trim() !== value) {
      issues.push(`versions[${index}] must not have surrounding whitespace`);
      return;
    }
    if (seen.has(value)) {
      issues.push(`versions[${index}] duplicates ${value}`);
      return;
    }
    seen.add(value);
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

function normalizeRelative(value) {
  if (
    typeof value !== 'string' ||
    value === '' ||
    value.trim() !== value ||
    value.includes('\\')
  ) {
    return null;
  }
  const normalized = value;
  const segments = normalized.split('/');
  if (
    path.isAbsolute(normalized) ||
    /^[A-Za-z]:/.test(normalized) ||
    segments.some(
      (segment) => segment === '' || segment === '.' || segment === '..',
    )
  ) {
    return null;
  }
  return normalized;
}

function normalizeArtifactPath(value) {
  if (typeof value !== 'string' || value.includes('\\')) {
    return null;
  }
  return normalizeRelative(value);
}

function isEd25519PublicKeyHex(value) {
  return /^[0-9a-f]{64}$/.test(value);
}

function isEd25519SignatureHex(value) {
  return /^[0-9a-f]{128}$/.test(value);
}

function isBlake3Hex(value) {
  return /^[0-9a-f]{64}$/.test(value ?? '');
}

function isNonEmptyString(value) {
  return typeof value === 'string' && value.trim() !== '';
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
