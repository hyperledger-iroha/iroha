// SPDX-License-Identifier: Apache-2.0

const SHA256_HEX = /^[0-9a-f]{64}$/;
const GIT_SHA1_HEX = /^[0-9a-f]{40}$/;

/**
 * Validate schema-compatible OpenAPI generator provenance.
 *
 * Legacy version-1 manifests without `generator_dirty` remain clean manifests.
 * Dirty development manifests must be unsigned, omit `generator_commit`, and
 * bind the exact non-generated source state with a lowercase SHA-256 digest.
 */
export function validateOpenApiGeneratorProvenance(
  manifest,
  {label = 'OpenAPI manifest', signed = false, requireClean = false} = {},
) {
  const dirtyField = manifest?.generator_dirty;
  if (dirtyField !== undefined && typeof dirtyField !== 'boolean') {
    throw new Error(`${label} generator_dirty must be boolean when present`);
  }
  const dirty = dirtyField === true;
  const commit = manifest?.generator_commit;
  const sourceDigest = manifest?.generator_source_sha256_hex;

  if (dirty) {
    if (commit !== null) {
      throw new Error(`${label} dirty provenance must set generator_commit to null`);
    }
    if (typeof sourceDigest !== 'string' || !SHA256_HEX.test(sourceDigest)) {
      throw new Error(
        `${label} dirty provenance requires generator_source_sha256_hex as 64 lowercase hexadecimal characters`,
      );
    }
    if (signed) {
      throw new Error(`${label} dirty provenance must not be signed`);
    }
    if (requireClean) {
      throw new Error(`${label} dirty provenance cannot be release-verified`);
    }
    return {dirty: true, commit: null, sourceSha256Hex: sourceDigest};
  }

  if (typeof commit !== 'string' || !GIT_SHA1_HEX.test(commit)) {
    throw new Error(
      `${label} clean provenance requires generator_commit as exactly 40 lowercase hexadecimal characters`,
    );
  }
  if (sourceDigest !== undefined && sourceDigest !== null) {
    throw new Error(`${label} clean provenance must omit generator_source_sha256_hex`);
  }
  return {dirty: false, commit, sourceSha256Hex: null};
}
