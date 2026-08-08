/** Exact first-release Musubi OpenAPI route and Norito model contract. */

export const MUSUBI_V1_MODELS = Object.freeze({
  '/v1/musubi/instructions/alias-register': ['RegisterMusubiAliasV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/alias-retarget': ['RetargetMusubiAliasV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/archive-location-add': ['AddMusubiArchiveLocationV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/archive-location-retire': ['RetireMusubiArchiveLocationV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/archive-register': ['RegisterMusubiArchiveV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/artifact-takedown': ['SetMusubiArtifactTakedownV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/namespace-binding-register': ['RegisterMusubiNamespaceBindingV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/package-member-accept': ['AcceptMusubiPackageMaintainerV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/package-member-invitation-revoke': ['RevokeMusubiPackageMaintainerInvitationV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/package-member-invite': ['InviteMusubiPackageMaintainerV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/package-member-remove': ['RemoveMusubiPackageMaintainerV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/package-member-set-role': ['SetMusubiPackageMaintainerRoleV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/package-metadata-set': ['SetMusubiPackageMetadataV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/package-recover': ['RecoverMusubiPackageV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/provider-bundle-attestation-register': ['RegisterMusubiProviderBundleAttestationV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/registry-policy-set': ['SetMusubiRegistryPolicyV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/release-digest-assert': ['AssertMusubiReleaseDigestV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/release-publish': ['PublishMusubiReleaseV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/instructions/release-yank-set': ['SetMusubiReleaseYankV1', 'MusubiInstructionEnvelopeV1'],
  '/v1/musubi/queries/alias': ['MusubiAliasQueryV1', 'MusubiAliasRecordV1'],
  '/v1/musubi/queries/alias-history': ['MusubiAliasQueryV1', 'MusubiAliasHistoryPageV1'],
  '/v1/musubi/queries/archive-locations': ['MusubiArchiveLocationQueryV1', 'MusubiArchiveLocationPageV1'],
  '/v1/musubi/queries/archive-retention': ['MusubiArchiveRetentionQueryV1', 'MusubiArchiveRetentionPageV1'],
  '/v1/musubi/queries/exact-package': ['MusubiExactPackageQueryV1', 'MusubiPackageRecordV1'],
  '/v1/musubi/queries/exact-release': ['MusubiExactReleaseQueryV1', 'MusubiExactReleaseSnapshotV1'],
  '/v1/musubi/queries/maintainers': ['MusubiPackagePageQueryV1', 'MusubiMaintainerPageV1'],
  '/v1/musubi/queries/ordered-prefix': ['MusubiOrderedPrefixQueryV1', 'MusubiOrderedPackagePageV1'],
  '/v1/musubi/queries/provider-bundle-attestation': ['MusubiProviderBundleAttestationKeyV1', 'MusubiProviderBundleAttestationRecordV1'],
  '/v1/musubi/queries/resolver-index': ['MusubiResolverIndexQueryV1', 'MusubiResolverIndexPageV1'],
  '/v1/musubi/queries/search': ['MusubiSearchQueryV1', 'MusubiSearchPageV1'],
  '/v1/musubi/queries/versions': ['MusubiPackagePageQueryV1', 'MusubiVersionPageV1'],
});
for (const models of Object.values(MUSUBI_V1_MODELS)) {
  Object.freeze(models);
}

export const MUSUBI_V1_PATHS = Object.freeze(Object.keys(MUSUBI_V1_MODELS).sort());

export const RETIRED_MUSUBI_PATHS = Object.freeze([
  '/v1/musubi/aliases/{alias}',
  '/v1/musubi/instructions/assert-release-exists',
  '/v1/musubi/instructions/publish-release',
  '/v1/musubi/instructions/set-alias',
  '/v1/musubi/instructions/yank-release',
  '/v1/musubi/packages',
  '/v1/musubi/release',
  '/v1/musubi/releases',
  '/v1/musubi/versions',
]);

function requireObject(value, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be a JSON object`);
  }
  return value;
}

function equalArrays(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

/** Validate one generated document against the exact 31-route Musubi V1 contract. */
export function verifyMusubiV1OpenApiContract(document, label = 'OpenAPI document') {
  if (MUSUBI_V1_PATHS.length !== 31) {
    throw new Error('internal Musubi V1 OpenAPI contract must contain exactly 31 routes');
  }
  const paths = requireObject(requireObject(document, label).paths, `${label}.paths`);
  const actualPaths = Object.keys(paths).filter((path) => path.startsWith('/v1/musubi/')).sort();
  if (!equalArrays(actualPaths, MUSUBI_V1_PATHS)) {
    const missing = MUSUBI_V1_PATHS.filter((path) => !actualPaths.includes(path));
    const extra = actualPaths.filter((path) => !MUSUBI_V1_PATHS.includes(path));
    throw new Error(`${label} has a stale Musubi route inventory (missing: ${missing.join(', ') || 'none'}; extra: ${extra.join(', ') || 'none'})`);
  }

  for (const path of MUSUBI_V1_PATHS) {
    const pathItem = requireObject(paths[path], `${label}.paths[${path}]`);
    const methods = Object.keys(pathItem);
    if (!equalArrays(methods, ['post'])) {
      throw new Error(`${label} ${path} must expose exactly one POST operation`);
    }
    const operation = requireObject(pathItem.post, `${label} POST ${path}`);
    const [requestType, responseType] = MUSUBI_V1_MODELS[path];
    if (!equalArrays(operation.tags ?? [], ['Musubi'])) {
      throw new Error(`${label} POST ${path} must carry only the Musubi tag`);
    }
    if (operation['x-iroha-norito-request-type'] !== requestType) {
      throw new Error(`${label} POST ${path} request model must be ${requestType}`);
    }
    if (operation['x-iroha-norito-response-type'] !== responseType) {
      throw new Error(`${label} POST ${path} response model must be ${responseType}`);
    }
    const expectedEffect = path.startsWith('/v1/musubi/queries/') ? 'read' : 'build_instruction';
    if (operation['x-iroha-tool-effect'] !== expectedEffect) {
      throw new Error(`${label} POST ${path} effect must be ${expectedEffect}`);
    }
  }

  for (const path of RETIRED_MUSUBI_PATHS) {
    if (Object.hasOwn(paths, path)) {
      throw new Error(`${label} retains retired Musubi path ${path}`);
    }
  }
}
