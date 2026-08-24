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

    assert.notEqual(result.status, 0, `${label} must reject non-Node-24 overrides`);
    assert.match(result.stdout, /^v26\.0\.0$/m, `${label} must print the selected Node version`);
    assert.match(result.stderr, /require Node 24/u, `${label} must explain the Node 24 gate`);
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

    assert.notEqual(result.status, 0, `${label} must reject non-Python-3.12 overrides`);
    assert.match(result.stdout, /^Python 3\.9\.6$/m, `${label} must print the selected Python version`);
    assert.match(result.stderr, /require Python 3\.12/u, `${label} must explain the Python 3.12 gate`);
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
    "--negative-control-swift-retired-request-field",
    "--negative-control-csharp-malformed-response-test",
    "--negative-control-kotlin-builder-test",
    "--negative-control-kotlin-successor-digest-test",
    "--negative-control-java-builder-test",
    "--negative-control-java-successor-digest-test",
    "--negative-control-rust-client-submitted-epoch",
    "--negative-control-js-submitted-epoch-type",
    "--negative-control-kotlin-submitted-epoch-model",
    "--negative-control-java-submitted-epoch-model",
  ]) {
    assert.match(workflow, new RegExp(`bash ci/check_sorafs_pin_register_sdk_guard\\.sh ${mode}`));
    assert.match(guard, new RegExp(mode));
  }
  assert.match(
    read("ci/check_sorafs_pin_register_js_sdk.sh"),
    /NODE_OVERRIDE="\$\{SORAFS_PIN_REGISTER_JS_SDK_NODE_BIN:-\}"[\s\S]*is_node_24_bin\(\)[\s\S]*resolve_node_24_bin\(\)[\s\S]*NODE_BIN="\$\(resolve_node_24_bin\)"[\s\S]*NODE_VERSION="\$\("\$\{NODE_BIN\}" --version\)"[\s\S]*printf '%s\\n' "\$\{NODE_VERSION\}"[\s\S]*v24\.\*\) ;;[\s\S]*registerSorafsPinManifest\|buildRegisterPinManifestInstruction\|rejects a retired submitted epoch\|SoraFS pin-register SDK guard\|SoraFS \.\* SDK runner/,
    "SoraFS JavaScript SDK runner must print the selected Node version and run runtime-gate meta tests",
  );
  assert.match(
    read("ci/check_sorafs_pin_register_python_sdk.sh"),
    /PYTHON_OVERRIDE="\$\{SORAFS_PIN_REGISTER_PYTHON_BIN:-\}"[\s\S]*resolve_python_312_bin\(\)[\s\S]*python3\.12[\s\S]*PYTHON_BIN="\$\(resolve_python_312_bin\)"/,
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
  assert.match(
    read("ci/check_sorafs_pin_register_csharp_sdk.sh"),
    /--filter-method "\*RegisterSoraFsPinManifestAsync\*"/,
    "SoraFS C# SDK runner must use the Microsoft Testing Platform method filter",
  );
  assert.doesNotMatch(
    read("ci/check_sorafs_pin_register_csharp_sdk.sh"),
    /FullyQualifiedName~RegisterSoraFsPinManifestAsync/,
    "SoraFS C# SDK runner must not use the ignored VSTest filter syntax",
  );
});

test("SoraFS JavaScript SDK runner rejects non-Node-24 overrides before tests", () => {
  assertRunnerRejectsNodeMajor(
    "ci/check_sorafs_pin_register_js_sdk.sh",
    "SORAFS_PIN_REGISTER_JS_SDK_NODE_BIN",
    "SoraFS JavaScript SDK runner",
  );
});

test("SoraFS Python SDK runner rejects non-3.12 overrides before tests", () => {
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

test("SoraFS pin-register SDK guard enforces caller-signed transport", () => {
  const src = read("javascript/iroha_js/src/toriiClient.js");
  const dist = read("javascript/iroha_js/dist/toriiClient.js");
  const transactionSrc = read("javascript/iroha_js/src/transaction.js");
  const transactionDist = read("javascript/iroha_js/dist/transaction.js");
  const indexSrc = read("javascript/iroha_js/src/index.js");
  const indexDist = read("javascript/iroha_js/dist/index.js");
  const nativeHost = read("crates/iroha_js_host/src/lib.rs");
  const dts = read("javascript/iroha_js/index.d.ts");
  const tests = read("javascript/iroha_js/test/toriiClient.test.js");
  const transactionTests = read("javascript/iroha_js/test/transactionBuilder.test.js");
  const pythonClient = read("python/iroha_python/src/iroha_python/client.py");
  const pythonTests = read("python/iroha_python/tests/client_sorafs_pin_register_test.py");
  const swiftClient = read("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift");
  const swiftTests = read("IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift");
  const csharpClient = read("csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiClient.cs");
  const csharpModels = read("csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiModels.cs");
  const csharpTests = read("csharp/tests/Hyperledger.Iroha.Sdk.Tests/ToriiClientTests.cs");
  const javaInstruction = read(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/RegisterPinManifestInstruction.java",
  );
  const kotlinInstruction = read(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstruction.kt",
  );

  for (const text of [src, dist]) {
    const method = text.match(
      /async registerSorafsPinManifest\(signedTransaction, options = \{\}\)[\s\S]*?(?=\n  \/\*\*)/,
    )?.[0];
    assert.ok(method, "signed pin-register method missing");
    assert.match(method, /"\/v1\/sorafs\/pin\/register"/);
    assert.match(method, /encodeCanonicalVersionedSignedTransactionV1/);
    assert.match(method, /"Content-Type": APPLICATION_NORITO/);
    assert.match(method, /Accept: APPLICATION_JSON/);
    assert.match(method, /_expectStatus\(response, \[202\]\)/);
    assert.doesNotMatch(method, /private_key|manifest_payload|JSON\.stringify/);
    assert.match(text, /normalizeSorafsPinRegisterResponse/);
    assert.doesNotMatch(text, /function buildSorafsPinRegisterPayload/);
    assert.doesNotMatch(
      text,
      /encodeSignedTransactionNorito|toVersionedTransactionPayload|unwrapNrt0NoritoFrame/,
    );
  }
  for (const text of [transactionSrc, transactionDist]) {
    assert.match(text, /function buildRegisterPinManifestInstruction\(input\)/);
    assert.match(text, /function buildRegisterPinManifestTransaction\(client, input, options = \{\}\)/);
    assert.match(text, /instructions: \[instruction\]/);
    assert.match(text, /quoteAndSignTransaction/);
    assert.match(text, /no longer accepts a submitted epoch/);
    assert.doesNotMatch(text, /submitTransactionEntrypoint/);
  }
  for (const text of [indexSrc, indexDist]) {
    assert.doesNotMatch(text, /submitTransactionEntrypoint/);
  }
  assert.match(nativeHost, /fn decode_canonical_signed_transaction_v1/);
  assert.doesNotMatch(
    nativeHost,
    /encode_signed_transaction_norito|try_decode_signed_transaction_adaptive_with_flags|try_decode_signed_transaction_versioned/,
  );
  assert.match(dts, /registerSorafsPinManifest\(/);
  assert.match(dts, /registerSorafsPinManifestTyped\(/);
  assert.match(dts, /signedTransaction: VersionedSignedTransactionV1/);
  assert.match(dts, /buildRegisterPinManifestInstruction/);
  assert.match(dts, /buildRegisterPinManifestTransaction/);
  assert.doesNotMatch(dts, /interface SorafsPinRegisterRequest/);
  assert.match(tests, /posts only an exact canonical V1 transaction/);
  assert.match(tests, /rejects legacy secret-bearing request objects/);
  assert.match(tests, /rejects pre-finality fee or custody claims/);
  assert.match(transactionTests, /quotes and signs exactly one instruction/);

  const pythonMethod = pythonClient.match(
    /    def register_sorafs_pin_manifest\([\s\S]*?(?=\n    def )/,
  )?.[0];
  assert.ok(pythonMethod, "Python signed pin-register method missing");
  assert.match(pythonMethod, /transaction: "SignedTransactionEnvelope"/);
  assert.match(pythonMethod, /transaction\.signed_transaction_versioned/);
  assert.match(pythonMethod, /"Content-Type": "application\/x-norito"/);
  assert.match(pythonMethod, /self\._expect_status\(response, \(202,\)\)/);
  assert.doesNotMatch(pythonMethod, /private_key|manifest_payload/);
  assert.match(pythonTests, /posts_only_versioned_signed_transaction/);
  assert.match(pythonTests, /rejects_pre_finality_fee_claim/);

  const swiftMethod = swiftClient.match(
    /    public func registerSoraFsPinManifest\(_ transaction: SignedTransactionEnvelope\) async throws[\s\S]*?(?=\n    public func getVpnProfile)/,
  )?.[0];
  assert.ok(swiftMethod, "Swift signed pin-register method missing");
  assert.match(swiftMethod, /body: transaction\.norito/);
  assert.match(swiftMethod, /"Content-Type": "application\/x-norito"/);
  assert.match(swiftMethod, /acceptedStatus: 202\.\.<203/);
  assert.doesNotMatch(swiftMethod, /private_key|manifestPayload/);
  assert.doesNotMatch(swiftClient, /struct ToriiSoraFsPinRegisterRequest/);
  assert.match(swiftTests, /testRegisterSoraFsPinManifestPostsOnlySignedNoritoAndReturnsAdmission/);
  assert.match(swiftTests, /testRegisterSoraFsPinManifestRejectsPreFinalityFeeClaims/);

  const csharpMethod = csharpClient.match(
    /    public async Task<ToriiSoraFsPinRegisterResponse> RegisterSoraFsPinManifestAsync\([\s\S]*?(?=\n    public Task<HttpResponseMessage> OpenSoraFsCidContentAsync)/,
  )?.[0];
  assert.ok(csharpMethod, "C# signed pin-register method missing");
  assert.match(csharpMethod, /SignedTransactionEnvelope transaction/);
  assert.match(csharpMethod, /transaction\.NoritoBytes/);
  assert.match(csharpMethod, /"application\/x-norito"/);
  assert.match(csharpMethod, /HttpStatusCode\.Accepted/);
  assert.doesNotMatch(csharpMethod, /private_key|ManifestPayload/);
  assert.doesNotMatch(csharpModels, /class ToriiSoraFsPinRegisterRequest/);
  const csharpResponse = csharpModels.match(
    /public sealed record class ToriiSoraFsPinRegisterResponse[\s\S]*?\n\}/,
  )?.[0];
  assert.ok(csharpResponse, "C# pin-register admission response missing");
  assert.doesNotMatch(csharpResponse, /PinFee|Custody|ChunkerHandle|Successor/);
  assert.match(csharpTests, /PostsOnlySignedNoritoAndReturnsAdmission/);
  assert.match(csharpTests, /RejectsNonAdmissionFields/);
  assert.match(csharpTests, /\[InlineData\("private_key"\)\]/);

  for (const text of [javaInstruction, kotlinInstruction]) {
    assert.match(text, /RegisterPinManifestInstruction/);
    assert.doesNotMatch(text, /submittedEpoch|submitted_epoch/);
    assert.match(text, /successorOfHex/);
    assert.doesNotMatch(text, /private[_A-Z]?key/i);
  }
});
