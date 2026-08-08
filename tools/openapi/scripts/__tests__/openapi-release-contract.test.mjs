import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

import {
  MUSUBI_V1_MODELS,
  MUSUBI_V1_PATHS,
  RETIRED_MUSUBI_PATHS,
  verifyMusubiV1OpenApiContract,
} from '../lib/musubi-v1-contract.mjs';

const testDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(testDir, '..', '..', '..', '..');

function exactMusubiDocument() {
  return {
    paths: Object.fromEntries(MUSUBI_V1_PATHS.map((path) => {
      const [requestType, responseType] = MUSUBI_V1_MODELS[path];
      return [path, {
        post: {
          tags: ['Musubi'],
          'x-iroha-norito-request-type': requestType,
          'x-iroha-norito-response-type': responseType,
          'x-iroha-tool-effect': path.startsWith('/v1/musubi/queries/')
            ? 'read'
            : 'build_instruction',
        },
      }];
    })),
  };
}

test('Musubi V1 contract verifier is exact and fail closed', () => {
  const exact = exactMusubiDocument();
  assert.doesNotThrow(() => verifyMusubiV1OpenApiContract(exact, 'fixture'));

  const missing = structuredClone(exact);
  delete missing.paths[MUSUBI_V1_PATHS[0]];
  assert.throws(
    () => verifyMusubiV1OpenApiContract(missing, 'missing fixture'),
    /stale Musubi route inventory/,
  );

  const wrongModel = structuredClone(exact);
  wrongModel.paths['/v1/musubi/queries/exact-package'].post[
    'x-iroha-norito-response-type'
  ] = 'LegacyMusubiPackage';
  assert.throws(
    () => verifyMusubiV1OpenApiContract(wrongModel, 'wrong-model fixture'),
    /response model must be MusubiPackageRecordV1/,
  );

  const retired = structuredClone(exact);
  retired.paths[RETIRED_MUSUBI_PATHS[0]] = {get: {}};
  assert.throws(
    () => verifyMusubiV1OpenApiContract(retired, 'retired fixture'),
    /stale Musubi route inventory/,
  );
});

test('checked OpenAPI artifacts expose only the Musubi V1 route contract', async () => {
  assert.equal(MUSUBI_V1_PATHS.length, 31);
  assert.deepEqual(Object.keys(MUSUBI_V1_MODELS).sort(), MUSUBI_V1_PATHS);

  for (const relativePath of [
    join('artifacts', 'openapi', 'torii.json'),
    join('artifacts', 'openapi', 'versions', 'current', 'torii.json'),
  ]) {
    const document = JSON.parse(await readFile(join(repoRoot, relativePath), 'utf8'));
    verifyMusubiV1OpenApiContract(document, relativePath);
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
  assert.ok(
    gate.includes('if (( ${#REPLAY_WORKTREES[@]} > 0 )); then'),
    'cleanup must guard empty-array expansion for Bash 3.2 with set -u',
  );
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
  const provisionerPath = [
    "    'tools',",
    "    'openapi',",
    "    'scripts',",
    "    'provision-openapi-cargo-lock.mjs',",
  ].join('\n');
  assert.ok(gate.includes(provisionerPath));
  assert.ok(!gate.includes("    'docs',\n    'portal',"));
  assert.match(gate, /const summary = await provisionOpenApiCargoLock\(\{/);
  assert.match(gate, /repoRoot: worktreeRoot,/);
  assert.match(gate, /summary\.status !== 'installed'/);
  assert.match(gate, /summary\.source !== 'operator'/);
  assert.match(gate, /summary\.path !== 'Cargo\.lock'/);
  assert.match(gate, /"\$\{REPO_ROOT\}\/Cargo\.lock"/);
  assert.doesNotMatch(gate, /cp "\$\{REPO_ROOT\}\/Cargo\.lock"/);
  for (const dependencyContract of [
    'stage_replay_openapi_dependencies()',
    'npm --prefix "${REPO_ROOT}/tools/openapi" ls --all --omit=dev --json',
    'cp -R "${source}/." "${target}/"',
    'diff -qr "${source}" "${target}"',
    'stage_replay_openapi_dependencies "${worktree}"',
  ]) {
    assert.ok(gate.includes(dependencyContract), dependencyContract);
  }
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
    'verify-musubi-v1-contract.test.mjs',
  ]) {
    assert.match(workflow, new RegExp(testFile.replaceAll('.', '\\.')));
  }
  assert.equal(
    Array.from(workflow.matchAll(/fetch-depth:\s*0/g)).length,
    3,
  );
  assert.match(workflow, /name: Complete Norito binding parity/);
  assert.match(workflow, /run: bash ci\/check_norito_bindings_sync\.sh/);
  for (const releaseInputTrigger of [
    '.cargo/config.toml',
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
