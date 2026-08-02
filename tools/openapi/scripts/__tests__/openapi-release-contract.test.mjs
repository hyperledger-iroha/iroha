import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const testDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(testDir, '..', '..', '..', '..');

const MUSUBI_V1_PATHS = [
  '/v1/musubi/instructions/alias-register',
  '/v1/musubi/instructions/alias-retarget',
  '/v1/musubi/instructions/archive-location-add',
  '/v1/musubi/instructions/archive-location-retire',
  '/v1/musubi/instructions/archive-register',
  '/v1/musubi/instructions/artifact-takedown',
  '/v1/musubi/instructions/namespace-binding-register',
  '/v1/musubi/instructions/package-member-accept',
  '/v1/musubi/instructions/package-member-invitation-revoke',
  '/v1/musubi/instructions/package-member-invite',
  '/v1/musubi/instructions/package-member-remove',
  '/v1/musubi/instructions/package-member-set-role',
  '/v1/musubi/instructions/package-metadata-set',
  '/v1/musubi/instructions/package-recover',
  '/v1/musubi/instructions/registry-policy-set',
  '/v1/musubi/instructions/release-digest-assert',
  '/v1/musubi/instructions/release-publish',
  '/v1/musubi/instructions/release-yank-set',
  '/v1/musubi/queries/alias',
  '/v1/musubi/queries/alias-history',
  '/v1/musubi/queries/archive-locations',
  '/v1/musubi/queries/archive-retention',
  '/v1/musubi/queries/exact-package',
  '/v1/musubi/queries/exact-release',
  '/v1/musubi/queries/maintainers',
  '/v1/musubi/queries/ordered-prefix',
  '/v1/musubi/queries/resolver-index',
  '/v1/musubi/queries/search',
  '/v1/musubi/queries/versions',
];

const MUSUBI_V1_MODELS = {
  '/v1/musubi/instructions/alias-register': [
    'RegisterMusubiAliasV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/alias-retarget': [
    'RetargetMusubiAliasV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/archive-location-add': [
    'AddMusubiArchiveLocationV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/archive-location-retire': [
    'RetireMusubiArchiveLocationV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/archive-register': [
    'RegisterMusubiArchiveV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/artifact-takedown': [
    'SetMusubiArtifactTakedownV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/namespace-binding-register': [
    'RegisterMusubiNamespaceBindingV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/package-member-accept': [
    'AcceptMusubiPackageMaintainerV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/package-member-invitation-revoke': [
    'RevokeMusubiPackageMaintainerInvitationV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/package-member-invite': [
    'InviteMusubiPackageMaintainerV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/package-member-remove': [
    'RemoveMusubiPackageMaintainerV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/package-member-set-role': [
    'SetMusubiPackageMaintainerRoleV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/package-metadata-set': [
    'SetMusubiPackageMetadataV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/package-recover': [
    'RecoverMusubiPackageV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/registry-policy-set': [
    'SetMusubiRegistryPolicyV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/release-digest-assert': [
    'AssertMusubiReleaseDigestV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/release-publish': [
    'PublishMusubiReleaseV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/instructions/release-yank-set': [
    'SetMusubiReleaseYankV1',
    'MusubiInstructionEnvelopeV1',
  ],
  '/v1/musubi/queries/alias': [
    'MusubiAliasQueryV1',
    'MusubiAliasRecordV1',
  ],
  '/v1/musubi/queries/alias-history': [
    'MusubiAliasQueryV1',
    'MusubiAliasHistoryPageV1',
  ],
  '/v1/musubi/queries/archive-locations': [
    'MusubiArchiveLocationQueryV1',
    'MusubiArchiveLocationPageV1',
  ],
  '/v1/musubi/queries/archive-retention': [
    'MusubiArchiveRetentionQueryV1',
    'MusubiArchiveRetentionPageV1',
  ],
  '/v1/musubi/queries/exact-package': [
    'MusubiExactPackageQueryV1',
    'MusubiPackageRecordV1',
  ],
  '/v1/musubi/queries/exact-release': [
    'MusubiExactReleaseQueryV1',
    'MusubiReleaseRecordV1',
  ],
  '/v1/musubi/queries/maintainers': [
    'MusubiPackagePageQueryV1',
    'MusubiMaintainerPageV1',
  ],
  '/v1/musubi/queries/ordered-prefix': [
    'MusubiOrderedPrefixQueryV1',
    'MusubiOrderedPackagePageV1',
  ],
  '/v1/musubi/queries/resolver-index': [
    'MusubiResolverIndexQueryV1',
    'MusubiResolverIndexPageV1',
  ],
  '/v1/musubi/queries/search': [
    'MusubiSearchQueryV1',
    'MusubiSearchPageV1',
  ],
  '/v1/musubi/queries/versions': [
    'MusubiPackagePageQueryV1',
    'MusubiVersionPageV1',
  ],
};

const RETIRED_MUSUBI_PATHS = [
  '/v1/musubi/aliases/{alias}',
  '/v1/musubi/instructions/assert-release-exists',
  '/v1/musubi/instructions/publish-release',
  '/v1/musubi/instructions/set-alias',
  '/v1/musubi/instructions/yank-release',
  '/v1/musubi/packages',
  '/v1/musubi/release',
  '/v1/musubi/releases',
  '/v1/musubi/versions',
];

test('checked OpenAPI artifacts expose only the Musubi V1 route contract', async () => {
  assert.equal(MUSUBI_V1_PATHS.length, 29);
  assert.deepEqual(Object.keys(MUSUBI_V1_MODELS).sort(), MUSUBI_V1_PATHS);

  for (const relativePath of [
    join('artifacts', 'openapi', 'torii.json'),
    join('artifacts', 'openapi', 'versions', 'current', 'torii.json'),
  ]) {
    const document = JSON.parse(await readFile(join(repoRoot, relativePath), 'utf8'));
    const paths = document.paths;
    assert.ok(paths && typeof paths === 'object' && !Array.isArray(paths));

    const musubiPaths = Object.keys(paths)
      .filter((path) => path.startsWith('/v1/musubi/'))
      .sort();
    assert.deepEqual(musubiPaths, MUSUBI_V1_PATHS, relativePath);

    for (const path of MUSUBI_V1_PATHS) {
      assert.deepEqual(Object.keys(paths[path]), ['post'], `${relativePath}: ${path}`);
      const operation = paths[path].post;
      const [requestType, responseType] = MUSUBI_V1_MODELS[path];
      assert.deepEqual(operation.tags, ['Musubi'], `${relativePath}: ${path} tag`);
      assert.equal(
        operation['x-iroha-norito-request-type'],
        requestType,
        `${relativePath}: ${path} request model`,
      );
      assert.equal(
        operation['x-iroha-norito-response-type'],
        responseType,
        `${relativePath}: ${path} response model`,
      );
      assert.equal(
        operation['x-iroha-tool-effect'],
        path.startsWith('/v1/musubi/queries/') ? 'read' : 'build_instruction',
        `${relativePath}: ${path} effect`,
      );
    }
    for (const retiredPath of RETIRED_MUSUBI_PATHS) {
      assert.equal(
        Object.hasOwn(paths, retiredPath),
        false,
        `${relativePath} retains retired Musubi path ${retiredPath}`,
      );
    }
  }
});

test('OpenAPI CI uses a locked graph and detached-only signing', async () => {
  const gate = await readFile(join(repoRoot, 'ci', 'check_openapi_spec.sh'), 'utf8');

  assert.match(gate, /cargo run \\\n\s+--locked \\\n\s+--offline \\/);
  assert.doesNotMatch(gate, /(?:^|\s)--sign(?:\s|\\|$)/m);
  assert.match(gate, /--unsigned-manifest --signing-payload/);
  assert.match(gate, /--signature-envelope/);
});

test('OpenAPI CI replays complete bundles from independent clean sources', async () => {
  const gate = await readFile(join(repoRoot, 'ci', 'check_openapi_spec.sh'), 'utf8');

  assert.match(gate, /require_clean_checkout/);
  assert.doesNotMatch(gate, /EXPECTED_GENERATOR_COMMIT/);
  assert.equal(
    Array.from(
      gate.matchAll(
        /node tools\/openapi\/scripts\/verify-openapi-release-inputs\.mjs/g,
      ),
    ).length,
    2,
  );
  assert.equal(
    Array.from(
      gate.matchAll(/python3 scripts\/check_sorafs_release_version_map\.py/g),
    ).length,
    2,
  );
  assert.match(
    gate,
    /build_unsigned_replay_bundle "\$\{REPLAY_WORKTREE_FIRST\}" "\$\{REPLAY_BUNDLE_FIRST\}"/,
  );
  assert.match(
    gate,
    /build_unsigned_replay_bundle "\$\{REPLAY_WORKTREE_SECOND\}" "\$\{REPLAY_BUNDLE_SECOND\}"/,
  );
  assert.match(gate, /create_replay_worktree "\$\{REPLAY_WORKTREE_FIRST\}"/);
  assert.match(gate, /create_replay_worktree "\$\{REPLAY_WORKTREE_SECOND\}"/);
  assert.match(
    gate,
    /REPLAY_COMMIT="\$\(git -C "\$\{REPO_ROOT\}" rev-parse --verify "HEAD\^\{commit\}"\)"/,
  );
  assert.match(
    gate,
    /worktree add --quiet --detach "\$\{worktree\}" "\$\{REPLAY_COMMIT\}"/,
  );
  assert.match(
    gate,
    /const sourcePath = await realpath\(sourceArgument\);/,
  );
  assert.match(gate, /provisionOpenApiCargoLock,/);
  assert.match(gate, /const summary = await provisionOpenApiCargoLock\(\{/);
  assert.match(gate, /repoRoot: worktreeRoot,/);
  assert.match(gate, /summary\.status !== 'installed'/);
  assert.match(gate, /summary\.source !== 'operator'/);
  assert.match(gate, /summary\.path !== 'Cargo\.lock'/);
  assert.match(gate, /"\$\{REPO_ROOT\}\/Cargo\.lock"/);
  assert.doesNotMatch(gate, /cp "\$\{REPO_ROOT\}\/Cargo\.lock"/);
  assert.match(gate, /REPLAY_CARGO_TARGET_DIR="\$\{TMP_DIR\}\/cargo-target"/);
  assert.doesNotMatch(gate, /REPLAY_CARGO_TARGET_DIR="\$\{REPO_ROOT\}/);
  assert.match(gate, /CARGO_TARGET_DIR="\$\{REPLAY_CARGO_TARGET_DIR\}"/);
  assert.doesNotMatch(
    gate,
    /allowedSignersFile: join\(outputDir, 'allowed_signers\.json'\)/,
  );
  assert.match(gate, /allowedSignersFile,/);
  assert.match(gate, /"\$\{ALLOWED_SIGNERS_PATH\}"/);
  assert.match(
    gate,
    /cp -R "\$\{REPLAY_BASELINE\}\/\." "\$\{output_dir\}\/"/,
  );
  assert.match(gate, /run_xtask_in_repo "\$\{source_root\}" openapi --unsigned-manifest/);
  assert.match(gate, /const \{syncOpenApi\} = await import\(syncModule\)/);
  assert.match(gate, /requireSigned: false/);
  assert.match(gate, /is not clean and unsigned/);
  const expectedGeneratedArtifacts = [
    'torii.json',
    'manifest.json',
    'versions/current/torii.json',
    'versions/current/manifest.json',
    'versions.json',
  ];
  const artifactBlock = gate.match(
    /GENERATED_RELEASE_ARTIFACTS=\(\n(?<artifacts>[\s\S]*?)\n\)/,
  );
  assert.ok(artifactBlock?.groups?.artifacts);
  assert.deepEqual(
    Array.from(
      artifactBlock.groups.artifacts.matchAll(/^\s+"([^"]+)"$/gm),
      (match) => match[1],
    ),
    expectedGeneratedArtifacts,
  );
  assert.match(gate, /diff -u "\$\{first\}" "\$\{second\}"/);
  assert.match(
    gate,
    /diff -ru "\$\{REPLAY_BUNDLE_FIRST\}" "\$\{REPLAY_BUNDLE_SECOND\}"/,
  );
  assert.match(
    gate,
    /diff -u "\$\{MANIFEST_PATH\}" "\$\{CURRENT_MANIFEST_PATH\}"/,
  );
  assert.match(
    gate,
    /diff -u "\$\{RELEASE_INPUT_SUMMARY_FIRST\}" "\$\{RELEASE_INPUT_SUMMARY_SECOND\}"/,
  );
  assert.match(
    gate,
    /diff -u "\$\{VERSION_MAP_SUMMARY_FIRST\}" "\$\{VERSION_MAP_SUMMARY_SECOND\}"/,
  );
  assert.doesNotMatch(gate, /VERSION_VERIFY_POLICY_ARGS/);
});

test('OpenAPI workflow actions and tooling tests are pinned', async () => {
  const workflow = await readFile(
    join(repoRoot, '.github', 'workflows', 'openapi.yml'),
    'utf8',
  );
  const actionReferences = Array.from(
    workflow.matchAll(/^\s*-\s+uses:\s+([^\s#]+)\s*$/gm),
    (match) => match[1],
  );

  assert.ok(actionReferences.length > 0);
  for (const reference of actionReferences) {
    assert.match(reference, /@[0-9a-f]{40}$/);
  }
  for (const testFile of [
    'openapi-manifest-v2.test.mjs',
    'openapi-release-inputs.test.mjs',
    'openapi-release-contract.test.mjs',
    'openapi-safe-file.test.mjs',
  ]) {
    assert.match(workflow, new RegExp(testFile.replaceAll('.', '\\.')));
  }
  assert.equal(
    Array.from(workflow.matchAll(/fetch-depth:\s*0/g)).length,
    2,
  );
  for (const releaseInputTrigger of [
    'Cargo.lock',
    'IrohaSwift/**',
    'Makefile',
    'ci/**',
    'ci/check_android_codegen.sh',
    'csharp/**',
    'data_model/**',
    'artifacts/openapi/**',
    'specs/sdk/android/generated/**',
    'fixtures/**',
    'integration_tests/**',
    'java/iroha_android/**',
    'java/norito_java/**',
    'javascript/iroha_js/**',
    'kotlin/**',
    'mochi/**',
    'python/iroha_python/**',
    'python/iroha_torii_client/**',
    'release/openapi-generator-inputs-v1.txt',
    'release/version-map.toml',
    'scripts/android_codegen_docs.py',
    'scripts/android_codegen_replay_sorafs_fixture.py',
    'scripts/check_android_codegen_parity.py',
    'scripts/check_sorafs_release_version_map.py',
    'scripts/**',
    'tools/**',
    'vendor/**',
  ]) {
    assert.match(
      workflow,
      new RegExp(`- "${releaseInputTrigger.replaceAll('.', '\\.').replaceAll('*', '\\*')}"`),
    );
  }
});
