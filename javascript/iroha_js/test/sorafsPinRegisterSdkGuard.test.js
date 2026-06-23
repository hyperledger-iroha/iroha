import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import test from "node:test";
import { fileURLToPath } from "node:url";

const REPO_ROOT = fileURLToPath(new URL("../../..", import.meta.url));

function read(path) {
  return readFileSync(new URL(path, `file://${REPO_ROOT}/`), "utf8");
}

function escapeRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function assertRunnerRejectsNodeMajor(script, envName, label) {
  const tmp = mkdtempSync(`${tmpdir()}/iroha-js-runner-node-`);
  const fakeNode = `${tmp}/node`;
  try {
    writeFileSync(
      fakeNode,
      [
        "#!/usr/bin/env bash",
        "if [[ \"${1:-}\" == \"--version\" ]]; then",
        "  printf '%s\\n' 'v26.0.0'",
        "  exit 0",
        "fi",
        "printf '%s\\n' \"unexpected fake node invocation: $*\" >&2",
        "exit 64",
        "",
      ].join("\n"),
    );
    chmodSync(fakeNode, 0o755);

    const result = spawnSync("bash", [script], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: { ...process.env, [envName]: fakeNode },
    });

    assert.notEqual(result.status, 0, `${label} must reject non-Node-20 overrides`);
    assert.match(result.stdout, /^v26\.0\.0$/m, `${label} must print the selected Node version`);
    assert.match(result.stderr, /require Node 20/u, `${label} must explain the Node 20 gate`);
    assert.doesNotMatch(
      result.stderr,
      /unexpected fake node invocation/u,
      `${label} must fail before running tests through the fake Node binary`,
    );
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function assertRunnerRejectsPythonMajor(script, envName, label) {
  const tmp = mkdtempSync(`${tmpdir()}/iroha-python-runner-`);
  const fakePython = `${tmp}/python3`;
  try {
    writeFileSync(
      fakePython,
      [
        "#!/usr/bin/env bash",
        "case \"${1:-}\" in",
        "  -c)",
        "    printf '%s\\n' '3.9'",
        "    exit 0",
        "    ;;",
        "  --version)",
        "    printf '%s\\n' 'Python 3.9.6'",
        "    exit 0",
        "    ;;",
        "esac",
        "printf '%s\\n' \"unexpected fake python invocation: $*\" >&2",
        "exit 64",
        "",
      ].join("\n"),
    );
    chmodSync(fakePython, 0o755);

    const result = spawnSync("bash", [script], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: { ...process.env, [envName]: fakePython },
    });

    assert.notEqual(result.status, 0, `${label} must reject non-Python-3.11 overrides`);
    assert.match(result.stdout, /^Python 3\.9\.6$/m, `${label} must print the selected Python version`);
    assert.match(result.stderr, /require Python 3\.11/u, `${label} must explain the Python 3.11 gate`);
    assert.doesNotMatch(
      result.stderr,
      /unexpected fake python invocation/u,
      `${label} must fail before venv setup or native builds`,
    );
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function assertRunnerPropagatesSwiftParseFailure(script, envName, label) {
  const tmp = mkdtempSync(`${tmpdir()}/iroha-swift-runner-`);
  const fakeSwiftc = `${tmp}/swiftc`;
  try {
    writeFileSync(
      fakeSwiftc,
      [
        "#!/usr/bin/env bash",
        "if [[ \"${1:-}\" == \"--version\" ]]; then",
        "  printf '%s\\n' 'Swift version 5.10.1 (fake)'",
        "  exit 0",
        "fi",
        "printf '%s\\n' \"fake swift parse failed: $*\" >&2",
        "exit 66",
        "",
      ].join("\n"),
    );
    chmodSync(fakeSwiftc, 0o755);

    const result = spawnSync("bash", [script], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: { ...process.env, [envName]: fakeSwiftc },
    });

    assert.notEqual(result.status, 0, `${label} must propagate swiftc parse failures`);
    assert.match(result.stdout, /Swift version 5\.10\.1 \(fake\)/u, `${label} must print swiftc version evidence`);
    assert.match(result.stderr, /fake swift parse failed:/u, `${label} must execute the parse command`);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function assertRunnerRejectsJavaHome(script, envName, label) {
  const tmp = mkdtempSync(`${tmpdir()}/iroha-jdk-runner-`);
  const binDir = `${tmp}/bin`;
  const fakeJava = `${binDir}/java`;
  try {
    mkdirSync(binDir, { recursive: true });
    writeFileSync(
      fakeJava,
      [
        "#!/usr/bin/env bash",
        "printf '%s\\n' 'openjdk version \"25.0.1\" 2026-01-01' >&2",
        "exit 0",
        "",
      ].join("\n"),
    );
    chmodSync(fakeJava, 0o755);

    const result = spawnSync("bash", [script], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: { ...process.env, [envName]: tmp },
    });

    assert.notEqual(result.status, 0, `${label} must reject non-JDK-21 homes`);
    assert.match(result.stderr, /JDK 21 home/u, `${label} must explain the JDK 21 gate`);
    assert.doesNotMatch(result.stderr, /gradle|javac/u, `${label} must fail before JVM tests or javac`);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function assertRunnerRejectsDotnetSdk(script, envName, label) {
  const tmp = mkdtempSync(`${tmpdir()}/iroha-dotnet-runner-`);
  const fakeDotnet = `${tmp}/dotnet`;
  try {
    writeFileSync(
      fakeDotnet,
      [
        "#!/usr/bin/env bash",
        "if [[ \"${1:-}\" == \"--version\" ]]; then",
        "  printf '%s\\n' '7.0.404'",
        "  exit 0",
        "fi",
        "printf '%s\\n' \"unexpected fake dotnet invocation: $*\" >&2",
        "exit 64",
        "",
      ].join("\n"),
    );
    chmodSync(fakeDotnet, 0o755);

    const result = spawnSync("bash", [script], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: { ...process.env, [envName]: fakeDotnet },
    });

    assert.notEqual(result.status, 0, `${label} must reject non-.NET-8 SDKs`);
    assert.match(result.stdout, /^7\.0\.404$/m, `${label} must print dotnet version evidence`);
    assert.match(result.stderr, /\.NET SDK 8\.0\.x/u, `${label} must explain the .NET 8 gate`);
    assert.doesNotMatch(
      result.stderr,
      /unexpected fake dotnet invocation/u,
      `${label} must fail before dotnet test`,
    );
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

function negativeControlModesFromInventory(text, startMarker, endMarker) {
  const start = text.indexOf(startMarker);
  assert.notEqual(start, -1, `missing ${startMarker}`);
  const end = text.indexOf(endMarker, start);
  assert.notEqual(end, -1, `missing ${endMarker}`);
  const modes = [...text.slice(start, end).matchAll(/(--negative-control-[A-Za-z0-9-]+)/gu)].map(
    (match) => match[1],
  );
  assert.equal(new Set(modes).size, modes.length, `${startMarker} must not duplicate modes`);
  return [...modes].sort();
}

function isPathInventoryString(value) {
  return (
    !/\s/u.test(value) &&
    (value.includes("/") || /\.(?:cs|h|java|js|json|kt|md|py|rs|sh|swift|toml|ts|yaml|yml)$/u.test(value))
  );
}

function quotedStringsFromInventory(text, startMarker, endMarker) {
  const start = text.indexOf(startMarker);
  assert.notEqual(start, -1, `missing ${startMarker}`);
  const end = text.indexOf(endMarker, start);
  assert.notEqual(end, -1, `missing ${endMarker}`);
  const paths = [...text.slice(start, end).matchAll(/"([^"]+)"/gu)]
    .map((match) => match[1])
    .filter(isPathInventoryString);
  assert.equal(new Set(paths).size, paths.length, `${startMarker} must not duplicate paths`);
  return [...paths].sort();
}

function assertWorkflowIncludesPaths(workflow, paths, label) {
  for (const path of paths) {
    assert.ok(
      new RegExp(`- "${escapeRegExp(path)}"`).test(workflow) ||
        workflow
          .match(/- "([^"]+\/\*\*)"/gu)
          ?.some((entry) => path.startsWith(entry.slice(3, -3))) === true,
      `${label} workflow paths must include ${path}`,
    );
  }
}

function assertWorkflowRunsNegativeControlModes(workflow, command, modes, label) {
  for (const mode of modes) {
    assert.match(
      workflow,
      new RegExp(`^\\s+${escapeRegExp(command)} ${escapeRegExp(mode)}$`, "m"),
      `${label} workflow must run ${mode}`,
    );
  }
}

test("SoraFS pin-register SDK guard locks required workflow lanes", () => {
  const workflow = read(".github/workflows/pr_sorafs_pin_register_sdk.yml");
  const guard = read("ci/check_sorafs_pin_register_sdk_guard.sh");

  for (const path of [
    "ci/check_no_tracked_python_bytecode.sh",
    "ci/check_sorafs_pin_register_js_sdk.sh",
    "ci/check_sorafs_pin_register_python_sdk.sh",
    "ci/check_sorafs_pin_register_jvm_sdk.sh",
    "ci/check_sorafs_pin_register_swift_sdk.sh",
    "ci/check_sorafs_pin_register_csharp_sdk.sh",
    "javascript/iroha_js/test/sorafsPinRegisterSdkGuard.test.js",
    "python/iroha_python/tests/client_sorafs_pin_register_test.py",
    "IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/ToriiClientTests.cs",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstructionTest.kt",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/SorafsRegisterPinManifestBuilderTests.java",
  ]) {
    assert.match(workflow, new RegExp(`- "${path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"`));
  }

  assert.match(
    workflow,
    /needs: \[sorafs_pin_register_swift_sdk_check, sorafs_pin_register_jvm_sdk_tests, sorafs_pin_register_csharp_sdk_tests, sorafs_pin_register_javascript_sdk_tests, sorafs_pin_register_python_sdk_tests\]/,
  );
  assert.match(workflow, /run: bash ci\/check_sorafs_pin_register_sdk_guard\.sh$/m);
  assert.match(
    workflow,
    /SoraFS pin-register SDK guard negative controls[\s\S]*Reject tracked Python bytecode[\s\S]*run:\s+bash ci\/check_no_tracked_python_bytecode\.sh[\s\S]*- name:\s+SoraFS pin-register SDK guard\s*\n\s*run:\s+bash ci\/check_sorafs_pin_register_sdk_guard\.sh/,
    "SoraFS workflow must reject tracked Python bytecode after negative controls and before the main guard",
  );
  assertWorkflowIncludesPaths(
    workflow,
    quotedStringsFromInventory(
      guard,
      "required_paths = (",
      "negative_control_commands = (",
    ),
    "SoraFS pin-register SDK guard",
  );
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "bash ci/check_sorafs_pin_register_sdk_guard.sh",
    negativeControlModesFromInventory(
      guard,
      "negative_control_commands = (",
      "class GuardError",
    ),
    "SoraFS pin-register SDK guard",
  );

  for (const mode of [
    "--negative-control-workflow-path",
    "--negative-control-bytecode-workflow",
    "--negative-control-js-sdk-needs-workflow",
    "--negative-control-jvm-sdk-distribution-workflow",
    "--negative-control-jvm-sdk-java-version-workflow",
    "--negative-control-jvm-sdk-setup-order-workflow",
    "--negative-control-jvm-sdk-java-home-override-script",
    "--negative-control-jvm-sdk-java-home-reject-script",
    "--negative-control-swift-sdk-version-script",
    "--negative-control-swift-sdk-override-script",
    "--negative-control-csharp-sdk-dotnet-version-workflow",
    "--negative-control-csharp-sdk-setup-order-workflow",
    "--negative-control-csharp-sdk-dotnet-version-script",
    "--negative-control-csharp-sdk-dotnet-override-script",
    "--negative-control-csharp-sdk-dotnet-major-script",
    "--negative-control-js-sdk-node-version-workflow",
    "--negative-control-js-sdk-node-version-script",
    "--negative-control-js-sdk-node-override-script",
    "--negative-control-js-sdk-node-resolver-script",
    "--negative-control-js-sdk-node-major-script",
    "--negative-control-js-sdk-node-cache-workflow",
    "--negative-control-js-sdk-node-setup-order-workflow",
    "--negative-control-python-sdk-version-workflow",
    "--negative-control-python-sdk-setup-order-workflow",
    "--negative-control-python-sdk-version-script",
    "--negative-control-python-sdk-override-script",
    "--negative-control-python-sdk-resolver-script",
    "--negative-control-python-sdk-major-script",
    "--negative-control-python-sdk-venv-rebuild-script",
    "--negative-control-python-sdk-bytecode-script",
    "--negative-control-python-adversarial-test",
    "--negative-control-swift-contract-test",
    "--negative-control-csharp-malformed-response-test",
    "--negative-control-kotlin-builder-test",
    "--negative-control-kotlin-chunker-unsigned-test",
    "--negative-control-java-builder-test",
    "--negative-control-java-chunker-unsigned-test",
  ]) {
    assert.match(workflow, new RegExp(`bash ci/check_sorafs_pin_register_sdk_guard\\.sh ${mode}`));
    assert.match(guard, new RegExp(mode));
  }
  assert.match(
    read("ci/check_sorafs_pin_register_js_sdk.sh"),
    /NODE_OVERRIDE="\$\{SORAFS_PIN_REGISTER_JS_SDK_NODE_BIN:-\}"[\s\S]*is_node_20_bin\(\)[\s\S]*resolve_node_20_bin\(\)[\s\S]*NODE_BIN="\$\(resolve_node_20_bin\)"[\s\S]*NODE_VERSION="\$\("\$\{NODE_BIN\}" --version\)"[\s\S]*printf '%s\\n' "\$\{NODE_VERSION\}"[\s\S]*v20\.\*\) ;;[\s\S]*registerSorafsPinManifest\|SoraFS pin-register SDK guard\|SoraFS \.\* SDK runner/,
    "SoraFS JavaScript SDK runner must print the selected Node version and run runtime-gate meta tests",
  );
  assert.match(
    read("ci/check_sorafs_pin_register_python_sdk.sh"),
    /PYTHON_OVERRIDE="\$\{SORAFS_PIN_REGISTER_PYTHON_BIN:-\}"[\s\S]*resolve_python_311_bin\(\)[\s\S]*python3\.11[\s\S]*PYTHON_BIN="\$\(resolve_python_311_bin\)"/,
    "SoraFS Python SDK runner must keep the documented Python override variable",
  );
  assert.match(
    read("ci/check_sorafs_pin_register_jvm_sdk.sh"),
    /JAVA_HOME_OVERRIDE="\$\{SORAFS_PIN_REGISTER_JVM_SDK_JAVA_HOME:-\}"/,
    "SoraFS JVM SDK runner must keep the documented Java home override variable",
  );
  assert.match(
    read("ci/check_sorafs_pin_register_jvm_sdk.sh"),
    /JAVA_HOME must point to a JDK 21 home for SoraFS pin-register JVM SDK tests\./,
    "SoraFS JVM SDK runner must reject inherited non-JDK-21 JAVA_HOME values",
  );
  assert.match(
    read("ci/check_sorafs_pin_register_swift_sdk.sh"),
    /SWIFTC_BIN="\$\{SORAFS_PIN_REGISTER_SWIFTC_BIN:-swiftc\}"/,
    "SoraFS Swift SDK runner must keep the documented swiftc override variable",
  );
  assert.match(
    read("ci/check_sorafs_pin_register_csharp_sdk.sh"),
    /DOTNET_BIN="\$\{SORAFS_PIN_REGISTER_DOTNET_BIN:-dotnet\}"/,
    "SoraFS C# SDK runner must keep the documented dotnet override variable",
  );
});

test("SoraFS JavaScript SDK runner rejects non-Node-20 overrides before tests", () => {
  assertRunnerRejectsNodeMajor(
    "ci/check_sorafs_pin_register_js_sdk.sh",
    "SORAFS_PIN_REGISTER_JS_SDK_NODE_BIN",
    "SoraFS JavaScript SDK runner",
  );
});

test("SoraFS Python SDK runner rejects non-3.11 overrides before tests", () => {
  assertRunnerRejectsPythonMajor(
    "ci/check_sorafs_pin_register_python_sdk.sh",
    "SORAFS_PIN_REGISTER_PYTHON_BIN",
    "SoraFS Python SDK runner",
  );
});

test("SoraFS Swift SDK runner propagates parse failures", () => {
  assertRunnerPropagatesSwiftParseFailure(
    "ci/check_sorafs_pin_register_swift_sdk.sh",
    "SORAFS_PIN_REGISTER_SWIFTC_BIN",
    "SoraFS Swift SDK runner",
  );
});

test("SoraFS JVM SDK runner rejects non-JDK-21 overrides before tests", () => {
  assertRunnerRejectsJavaHome(
    "ci/check_sorafs_pin_register_jvm_sdk.sh",
    "SORAFS_PIN_REGISTER_JVM_SDK_JAVA_HOME",
    "SoraFS JVM SDK runner",
  );
});

test("SoraFS C# SDK runner rejects non-.NET-8 overrides before tests", () => {
  assertRunnerRejectsDotnetSdk(
    "ci/check_sorafs_pin_register_csharp_sdk.sh",
    "SORAFS_PIN_REGISTER_DOTNET_BIN",
    "SoraFS C# SDK runner",
  );
});

test("SoraFS pin-register SDK guard exposes typed JavaScript helpers", () => {
  const src = read("javascript/iroha_js/src/toriiClient.js");
  const dist = read("javascript/iroha_js/dist/toriiClient.js");
  const dts = read("javascript/iroha_js/index.d.ts");
  const tests = read("javascript/iroha_js/test/toriiClient.test.js");
  const pythonClient = read("python/iroha_python/src/iroha_python/client.py");
  const pythonTests = read("python/iroha_python/tests/client_sorafs_pin_register_test.py");
  const swiftClient = read("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift");
  const swiftTests = read("IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift");
  const csharpClient = read("csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiClient.cs");
  const csharpModels = read("csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiModels.cs");
  const csharpTests = read("csharp/tests/Hyperledger.Iroha.Sdk.Tests/ToriiClientTests.cs");

  for (const text of [src, dist]) {
    assert.match(text, /async registerSorafsPinManifest\(input = \{\}\)/);
    assert.match(text, /"\/v1\/sorafs\/pin\/register"/);
    assert.match(text, /buildSorafsPinRegisterPayload/);
    assert.match(text, /normalizeSorafsPinRegisterResponse/);
    assert.match(text, /ambiguous aliases/);
    assert.match(text, /"manifestBytes"/);
    assert.match(text, /"manifest_b64"/);
    assert.match(text, /payload\.manifest_b64 = normalizeRequiredBase64Payload/);
  }
  assert.match(dts, /registerSorafsPinManifest\(/);
  assert.match(dts, /registerSorafsPinManifestTyped\(/);
  assert.match(dts, /manifestBytes\?: BinaryLike \| string \| null;/);
  assert.match(dts, /manifest_b64\?: BinaryLike \| string \| null;/);
  assert.match(
    tests,
    /registerSorafsPinManifest rejects ambiguous request field aliases before fetch/,
  );
  assert.match(tests, /registerSorafsPinManifest rejects malformed manifest payload before fetch/);
  assert.match(tests, /registerSorafsPinManifestTyped rejects ambiguous response aliases/);

  assert.match(pythonClient, /"manifest_b64"/);
  assert.match(pythonClient, /"manifestBytes"/);
  assert.match(pythonClient, /accepts only one of manifest_b64 or manifest_bytes/);
  assert.match(pythonTests, /"manifestBytes": b"manifest-norito"/);
  assert.match(pythonTests, /body\["manifest_b64"\]/);

  assert.match(swiftClient, /public var manifestBase64: String\?/);
  assert.match(swiftClient, /public var manifestBytes: Data\?/);
  assert.match(swiftClient, /case manifestBase64 = "manifest_b64"/);
  assert.match(swiftClient, /optionalManifestPayload\(manifestBase64: String\?, manifestBytes: Data\?\)/);
  assert.match(swiftTests, /testRegisterSoraFsPinManifestAcceptsManifestBase64Payload/);
  assert.match(swiftTests, /root\["manifest_b64"\]/);

  assert.match(csharpClient, /NormalizeOptionalSoraFsManifestPayload/);
  assert.match(csharpClient, /Convert\.ToBase64String\(manifestBytes\)/);
  assert.match(csharpClient, /Provide either ManifestBase64 or ManifestBytes, not both\./);
  assert.match(csharpModels, /public string\? ManifestBase64/);
  assert.match(csharpModels, /public byte\[\]\? ManifestBytes/);
  assert.match(csharpModels, /\[JsonPropertyName\("manifest_b64"\)\]/);
  assert.match(csharpTests, /RegisterSoraFsPinManifestAsyncAcceptsManifestBase64Payload/);
  assert.match(csharpTests, /GetProperty\("manifest_b64"\)/);
});
