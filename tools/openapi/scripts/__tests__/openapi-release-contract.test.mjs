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

const forbiddenProcessControlPatterns = [
  /^[ \t]*(?:command[ \t]+)?(?:kill|killall|pkill|renice|timeout)(?:[ \t]|$)/m,
  /\b(?:os\.kill|(?:os\.)?killpg)\s*\(/,
  /\.\s*(?:terminate|kill)\s*\(/,
  /\bstart_new_session\s*=/,
  /\b(?:SIG)?(?:STOP|TERM|KILL)\b/,
  /\.\s*(?:wait|communicate)\s*\([^)]*\btimeout\s*=/s,
  /\bsubprocess\.(?:run|call|check_call|check_output)\s*\([^)]*\btimeout\s*=/s,
];

function forbiddenProcessControls(source) {
  return forbiddenProcessControlPatterns.filter((pattern) => pattern.test(source));
}

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

test('OpenAPI Cargo paths use the shared no-interference policy', async () => {
  const gate = await readFile(join(repoRoot, 'ci', 'check_openapi_spec.sh'), 'utf8');
  const generator = await readFile(
    join(repoRoot, 'ci', 'run_openapi_generator.sh'),
    'utf8',
  );
  const android = await readFile(
    join(repoRoot, 'ci', 'check_android_codegen.sh'),
    'utf8',
  );
  const policy = await readFile(
    join(repoRoot, 'scripts', 'sumeragi_v2_release_process_policy.sh'),
    'utf8',
  );

  for (const source of [gate, generator]) {
    assert.match(source, /source "\$\{PROCESS_POLICY\}"/);
    assert.match(source, /run_cargo run[\s\S]*?--locked[\s\S]*?--offline/);
    assert.doesNotMatch(source, /\bcargo(?:\s|\+)/);
    assert.doesNotMatch(source, /(?:^|\s)--jobs(?:=|\s)|(?:^|\s)-j\d*/m);
    assert.doesNotMatch(source, /(?:^|\s)--sign(?:\s|\\|$)/m);
  }
  for (const source of [gate, generator, android]) {
    assert.match(source, /compgen -e/);
    assert.match(source, /unset "\$\{openapi_git_variable\}"/);
    assert.match(source, /export GIT_OPTIONAL_LOCKS=0/);
    for (const setting of [
      'GIT_NO_LAZY_FETCH=1',
      'GIT_NO_REPLACE_OBJECTS=1',
      'GIT_CONFIG_NOSYSTEM=1',
      'GIT_CONFIG_GLOBAL=/dev/null',
      'GIT_CONFIG_COUNT=2',
      'GIT_CONFIG_KEY_0=core.hooksPath',
      'GIT_CONFIG_VALUE_0=/dev/null',
      'GIT_CONFIG_KEY_1=core.fsmonitor',
      'GIT_CONFIG_VALUE_1=false',
    ]) {
      assert.ok(source.includes(setting), setting);
    }
    assert.ok(source.indexOf('compgen -e') < source.indexOf('git -C'));
  }
  for (const relativePath of [
    join('ci', 'check_openapi_spec.sh'),
    join('ci', 'run_openapi_generator.sh'),
    join('scripts', 'seal_workspace_source.py'),
    join('scripts', 'check_sorafs_release_version_map.py'),
    join('tools', 'openapi', 'scripts', 'provision-openapi-cargo-lock.mjs'),
    join('tools', 'openapi', 'scripts', 'sync-openapi.mjs'),
    join('tools', 'openapi', 'scripts', 'verify-musubi-v1-contract.mjs'),
    join('tools', 'openapi', 'scripts', 'verify-openapi-versions.mjs'),
    join('tools', 'openapi', 'scripts', 'verify-openapi-release-inputs.mjs'),
    join('tools', 'openapi', 'scripts', 'check-openapi-signatures.mjs'),
  ]) {
    const source = await readFile(join(repoRoot, relativePath), 'utf8');
    assert.deepEqual(forbiddenProcessControls(source), [], relativePath);
  }
  assert.match(policy, /acquire_invocation_cargo_lock\(\) \{/);
  assert.match(policy, /release_invocation_cargo_lock\(\) \{/);
  assert.match(policy, /lock\.mkdir\(mode=0o700\)/);
  assert.doesNotMatch(
    policy,
    /wait_for_external_cargo|\bps\s+-|pgrep|\/proc\/|process_snapshot|\bsleep\s+/,
  );
  assert.match(
    policy,
    /if "\$IROHA_RELEASE_CARGO_BIN" "\$@"; then/,
  );
  assert.match(
    policy,
    /_run_cargo_with_scoped_lock "\$label" "\$\{pinned_arguments\[@\]\}"/,
  );
  assert.match(policy, /pinned_arguments=\("\$subcommand" -j1\)/);
  assert.match(policy, /pinned_arguments\+=\("\$@"\)/);
  assert.doesNotMatch(policy, /local status/);
  assert.match(policy, /locked_count != 1 \|\| offline_count != 1/);
  assert.match(gate, /--unsigned-manifest[\s\S]*?--signing-payload/);
  assert.match(gate, /--signature-envelope/);
  assert.match(generator, /--unsigned-manifest/);
  assert.match(generator, /--signature-envelope/);

  for (const source of [gate, generator]) {
    assert.match(
      source,
      /if \[\[ -z "\$\{IROHA_RELEASE_ARTIFACT_ROOT:-\}" \\\n  && -z "\$\{IROHA_RELEASE_CANCEL_REQUEST_PATH:-\}" \]\]; then[\s\S]*?elif \[\[ -z "\$\{IROHA_RELEASE_ARTIFACT_ROOT:-\}" \\\n  \|\| -z "\$\{IROHA_RELEASE_CANCEL_REQUEST_PATH:-\}" \]\]; then/,
    );
    assert.match(
      source,
      /IROHA_RELEASE_ARTIFACT_ROOT and IROHA_RELEASE_CANCEL_REQUEST_PATH must be supplied together/,
    );
    assert.match(source, /release cancellation marker parent/);
    assert.match(source, /require_external_release_artifact_root/);
    assert.match(source, /require_disjoint_release_roots/);
  }
  assert.equal(
    Array.from(gate.matchAll(/require_disjoint_release_roots/g)).length,
    2,
  );
  assert.equal(
    Array.from(generator.matchAll(/require_disjoint_release_roots/g)).length,
    1,
  );
  assert.match(
    generator,
    /IROHA_RELEASE_CANCEL_REQUEST_PATH="\$\{IROHA_RELEASE_ARTIFACT_ROOT%\/\*\}\/cancel-request\.json"/,
  );
  assert.match(gate, /release_gate_boundary "openapi:channels-ready"/);
  assert.match(gate, /OpenAPI authenticated artifact root/);
  assert.match(gate, /OpenAPI cooperative cancellation marker/);
  assert.match(
    gate,
    /OPENAPI_EVIDENCE_DIR="\$\(mktemp -d "\$\{IROHA_RELEASE_ARTIFACT_ROOT\}\/openapi-check\.XXXXXX"\)"/,
  );
  assert.match(generator, /release_gate_boundary "openapi-generator:channels-ready"/);
  assert.match(generator, /OpenAPI generator authenticated artifact root/);
  assert.match(generator, /OpenAPI generator cooperative cancellation marker/);
  assert.match(generator, /require_release_artifact_directory "\$\{OUTPUT_DIR\}"/);
  assert.match(
    generator,
    /require_release_artifact_directory "\$\{SIGNING_PAYLOAD%\/\*\}"/,
  );
});

test('OpenAPI process-control scan rejects reachable spelling mutations', () => {
  for (const mutation of [
    'kill "$child_pid"',
    'pkill -STOP cargo',
    'renice 10 "$child_pid"',
    'timeout 30 command cargo',
    'os.kill(child_pid, 9)',
    'os.killpg(group_id, 9)',
    'killpg(group_id, 9)',
    'child.terminate()',
    'child.kill()',
    'subprocess.Popen(command, start_new_session=True)',
    'child.wait(timeout = 30)',
    'child.communicate(\n  timeout=30)',
    'subprocess.run(command,\n  timeout=30)',
    'signal.SIGSTOP',
    'signal.SIGTERM',
    'signal.SIGKILL',
  ]) {
    assert.notDeepEqual(forbiddenProcessControls(`safe_prefix\n${mutation}\n`), []);
  }
  assert.deepEqual(
    forbiddenProcessControls(
      '# Cooperative cancellation is observed only at a gate boundary.\n',
    ),
    [],
  );
});

test('OpenAPI CI replays complete bundles from independent clean sources', async () => {
  const gate = await readFile(join(repoRoot, 'ci', 'check_openapi_spec.sh'), 'utf8');

  assert.match(gate, /require_clean_checkout/);
  assert.doesNotMatch(gate, /worktree add|worktree remove|rm -rf/);
  assert.doesNotMatch(gate, /EXPECTED_GENERATOR_COMMIT/);
  assert.equal(
    Array.from(
      gate.matchAll(
        /tools\/openapi\/scripts\/verify-openapi-release-inputs\.mjs/g,
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
    /build_unsigned_replay_bundle[\s\S]*?"\$\{REPLAY_SOURCE_FIRST\}"[\s\S]*?"\$\{REPLAY_CARGO_TARGET_DIR_FIRST\}"[\s\S]*?"\$\{REPLAY_GENERATED_FIRST\}"[\s\S]*?"\$\{REPLAY_BUNDLE_FIRST\}"/,
  );
  assert.match(
    gate,
    /build_unsigned_replay_bundle[\s\S]*?"\$\{REPLAY_SOURCE_SECOND\}"[\s\S]*?"\$\{REPLAY_CARGO_TARGET_DIR_SECOND\}"[\s\S]*?"\$\{REPLAY_GENERATED_SECOND\}"[\s\S]*?"\$\{REPLAY_BUNDLE_SECOND\}"/,
  );
  assert.match(gate, /create_replay_source "\$\{REPLAY_SOURCE_FIRST\}"/);
  assert.match(gate, /create_replay_source "\$\{REPLAY_SOURCE_SECOND\}"/);
  assert.match(
    gate,
    /REPLAY_COMMIT="\$\(git -C "\$\{REPO_ROOT\}" rev-parse --verify "HEAD\^\{commit\}"\)"/,
  );
  assert.match(
    gate,
    /REPLAY_TREE="\$\(git -C "\$\{REPO_ROOT\}" rev-parse --verify "\$\{REPLAY_COMMIT\}\^\{tree\}"\)"/,
  );
  assert.match(gate, /git clone --quiet --local --no-hardlinks --no-checkout/);
  assert.match(gate, /checkout --quiet --detach "\$\{REPLAY_COMMIT\}"/);
  assert.match(gate, /--seal --root "\$\{source_root\}" --no-writable-paths/);
  assert.match(gate, /--verify --root "\$\{source_root\}" --no-writable-paths/);
  assert.match(gate, /actual_tree=.*"HEAD\^\{tree\}"/);
  assert.match(gate, /"\$\{actual_tree\}" != "\$\{REPLAY_TREE\}"/);
  assert.match(gate, /const sourcePath = await realpath\(lockSourceArgument\);/);
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
  assert.match(gate, /repoRoot: replaySourceRoot,/);
  assert.match(gate, /summary\.status !== 'verified'/);
  assert.match(gate, /summary\.source !== 'tracked'/);
  assert.match(gate, /summary\.path !== 'Cargo\.lock'/);
  assert.match(gate, /"\$\{REPO_ROOT\}\/Cargo\.lock"/);
  assert.doesNotMatch(gate, /cp "\$\{REPO_ROOT\}\/Cargo\.lock"/);
  for (const dependencyContract of [
    'stage_replay_openapi_dependencies()',
    'local source="${OPENAPI_NODE_MODULES_ROOT}"',
    '"${source_root}/tools/openapi/package.json"',
    '"${source_root}/tools/openapi/package-lock.json"',
    '"${target}/.package-lock.json"',
    'source_packages[""] != package_policy',
    '{name: value for name, value in source_packages.items() if name}',
    'cp -R "${source}/." "${target}/"',
    'diff -qr "${source}" "${target}"',
    'stage_replay_openapi_dependencies "${source_root}"',
  ]) {
    assert.ok(gate.includes(dependencyContract), dependencyContract);
  }
  assert.ok(!gate.includes('npm --prefix'));
  assert.ok(gate.includes('OPENAPI_NODE_BIN="${OPENAPI_NODE_BIN:-}"'));
  assert.ok(gate.includes('"${OPENAPI_NODE_BIN}" --input-type=module -'));
  assert.ok(gate.includes('OPENAPI_DEPENDENCY_STATE_BEFORE="$(openapi_dependency_state)"'));
  assert.ok(gate.includes('OPENAPI_DEPENDENCY_STATE_AFTER="$(openapi_dependency_state)"'));
  assert.ok(gate.includes('openapi_dependency_state "${REPLAY_SOURCE_FIRST}/tools/openapi/node_modules"'));
  assert.ok(gate.includes('openapi_dependency_state "${REPLAY_SOURCE_SECOND}/tools/openapi/node_modules"'));
  assert.ok(gate.includes('identity(os.fstat(descriptor)) != fingerprint'));
  assert.ok(gate.includes('before.st_mtime_ns, before.st_ctime_ns'));
  assert.match(
    gate,
    /OPENAPI_RUN_ROOT="\$\(mktemp -d \/private\/tmp\/iroha-openapi-check\.XXXXXX\)"/,
  );
  assert.match(
    gate,
    /REPLAY_CARGO_TARGET_DIR_FIRST="\$\{OPENAPI_RUN_ROOT\}\/target-first"/,
  );
  assert.match(
    gate,
    /REPLAY_CARGO_TARGET_DIR_SECOND="\$\{OPENAPI_RUN_ROOT\}\/target-second"/,
  );
  assert.match(gate, /CARGO_TARGET_DIR="\$\{target_root\}"/);
  assert.match(gate, /require_external_private_directory/);
  assert.match(gate, /require_external_release_artifact_root/);
  assert.match(gate, /require_release_artifact_directory/);
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
  assert.match(
    gate,
    /--output-root "\$\{generated_dir\}"[\s\S]*?--unsigned-manifest/,
  );
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
  assert.match(gate, /source-identity\.json/);
  assert.match(gate, /"candidate_commit": commit/);
  assert.match(gate, /"candidate_tree": tree/);
  const beforeCompletion = gate.indexOf(
    'release_gate_boundary "openapi:before-completion-publication"',
  );
  const completionReceipt = gate.indexOf(
    '"${OPENAPI_EVIDENCE_DIR}/source-identity.json"',
  );
  const afterCompletion = gate.indexOf(
    'release_gate_boundary "openapi:after-completion-publication"',
  );
  assert.ok(beforeCompletion < completionReceipt);
  assert.ok(completionReceipt < afterCompletion);
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
  assert.match(workflow, /cancel-in-progress: false/);
  assert.doesNotMatch(workflow, /cancel-in-progress: true/);
  assert.doesNotMatch(workflow, /timeout-minutes:/);
  assert.doesNotMatch(workflow, /CARGO_TARGET_DIR:/);
  assert.doesNotMatch(workflow, /Swatinem\/rust-cache/);
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

test('OpenAPI owner commands stage out of tree through the compliant wrapper', async () => {
  const readme = await readFile(join(repoRoot, 'tools', 'openapi', 'README.md'), 'utf8');
  const registry = await readFile(join(repoRoot, 'generated-files.toml'), 'utf8');
  const bundleEntry = registry.match(
    /\[\[generated\]\]\nname = "torii-openapi-release-bundle"[\s\S]*?(?=\n\[\[generated\]\])/,
  );

  assert.ok(bundleEntry);
  for (const source of [readme, bundleEntry[0]]) {
    assert.match(source, /bash ci\/run_openapi_generator\.sh/);
    assert.match(source, /--output-dir/);
    assert.doesNotMatch(source, /\bcargo(?:\s|\+)/);
    assert.doesNotMatch(source, /--lockfile-path|unstable-options|(?:^|\s)-Z(?:\s|$)/m);
    assert.doesNotMatch(
      source,
      /openapi --output-root (?:artifacts\/openapi|"?\$PWD\/artifacts\/openapi)/,
    );
  }
  assert.match(readme, /mktemp -d \/private\/tmp\/iroha-openapi-refresh\.XXXXXX/);
  assert.match(
    readme,
    /OPENAPI_ARTIFACT_ROOT="\$\{OPENAPI_RUN_ROOT\}\/artifacts"/,
  );
  assert.match(
    readme,
    /OPENAPI_STAGE="\$\{OPENAPI_ARTIFACT_ROOT\}\/openapi"/,
  );
  assert.match(
    readme,
    /export IROHA_RELEASE_ARTIFACT_ROOT="\$\{OPENAPI_ARTIFACT_ROOT\}"/,
  );
  assert.match(
    readme,
    /export IROHA_RELEASE_CANCEL_REQUEST_PATH="\$\{OPENAPI_RUN_ROOT\}\/cancel-request\.json"/,
  );
  assert.match(readme, /must provide both/);
  assert.match(
    bundleEntry[0],
    /IROHA_OPENAPI_STAGE:\?set an existing absolute private \/private\/tmp <run>\/artifacts\/<stage>/,
  );
  assert.match(
    bundleEntry[0],
    /IROHA_RELEASE_ARTIFACT_ROOT=\\"\$\{IROHA_OPENAPI_STAGE%\/\*\}\\"/,
  );
  assert.match(
    bundleEntry[0],
    /IROHA_RELEASE_CANCEL_REQUEST_PATH=\\"\$\{IROHA_OPENAPI_STAGE%\/\*\/\*\}\/cancel-request\.json\\"/,
  );
  for (const sourcePath of [
    'ci/check_openapi_spec.sh',
    'ci/run_openapi_generator.sh',
    'scripts/seal_workspace_source.py',
    'scripts/sumeragi_v2_release_process_policy.sh',
  ]) {
    assert.match(bundleEntry[0], new RegExp(sourcePath.replaceAll('.', '\\.')));
  }
});
