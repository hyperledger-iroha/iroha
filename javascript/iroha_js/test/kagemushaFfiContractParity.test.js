import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import test from "node:test";
import { fileURLToPath } from "node:url";

const REPO_ROOT = fileURLToPath(new URL("../../..", import.meta.url));

const REQUIRED_C_SYMBOLS = Object.freeze([
  "connect_norito_kagemusha_recursive_spend_init",
  "connect_norito_kagemusha_recursive_spend_append",
  "connect_norito_kagemusha_recursive_spend_transition_profile_init",
  "connect_norito_kagemusha_recursive_spend_transition_profile_append",
  "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
  "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
  "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
  "connect_norito_kagemusha_recursive_spend_verify",
  "connect_norito_kagemusha_recursive_spend_redeem",
  "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle",
]);

const REQUIRED_RECURSIVE_COMPACT_C_SYMBOLS = Object.freeze([
  "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes",
  "connect_norito_kagemusha_verify_recursive_compact_payment_token",
  "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle",
  "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection",
  "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height",
]);

const REQUIRED_JS_NATIVE_METHODS = Object.freeze([
  "kagemushaRecursiveSpendInit",
  "kagemushaRecursiveSpendAppend",
  "kagemushaRecursiveSpendTransitionProfileInit",
  "kagemushaRecursiveSpendTransitionProfileAppend",
  "kagemushaRecursiveSpendLineageAppendBoundary",
  "kagemushaRecursiveSpendLineageWitnessFromInitResult",
  "kagemushaRecursiveSpendLineageWitnessAppendResult",
  "kagemushaRecursiveSpendVerify",
  "kagemushaRecursiveSpendRedeem",
]);

const REQUIRED_RECURSIVE_COMPACT_JS_METHODS = Object.freeze([
  "kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
  "kagemushaVerifyRecursiveCompactPaymentToken",
  "kagemushaRecursiveSpendCompactPaymentTokenFromBundle",
  "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection",
]);

const REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS = Object.freeze([
  "kagemushaProveVerifiedCompactPaymentTokenWithRecords",
  "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
]);

const REQUIRED_KAGEMUSHA_PALLAS_OPEN_ENVELOPE_BUILDER_JS_METHODS = Object.freeze([
  "kagemushaBuildPallasOpenEnvelopesArchive",
  "kagemushaBuildPreviousProofOpenEnvelopesArchive",
]);

const REQUIRED_PYTHON_NATIVE_METHODS = Object.freeze([
  "kagemusha_recursive_spend_init",
  "kagemusha_recursive_spend_append",
  "kagemusha_recursive_spend_transition_profile_init",
  "kagemusha_recursive_spend_transition_profile_append",
  "kagemusha_recursive_spend_lineage_append_boundary",
  "kagemusha_recursive_spend_lineage_witness_from_init_result",
  "kagemusha_recursive_spend_lineage_witness_append_result",
  "kagemusha_recursive_spend_verify",
  "kagemusha_recursive_spend_redeem",
]);

const REQUIRED_RECURSIVE_COMPACT_PYTHON_METHODS = Object.freeze([
  "kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes",
  "kagemusha_verify_recursive_compact_payment_token",
  "kagemusha_recursive_spend_compact_payment_token_from_bundle",
]);

const REQUIRED_HEADER_NEGATIVE_CONTROL_MODES = Object.freeze([
  "--negative-control-missing-recursive-header",
  "--negative-control-bad-recursive-signature",
  "--negative-control-missing-rust-export",
  "--negative-control-umbrella-drift",
]);

const APPEND_OPENINGS_PREFLIGHT_DOMAIN =
  "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1";
const APPEND_BOUNDARY_DOMAIN =
  "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1";
const APPEND_BOUNDARY_CHAIN_ASSET_DOMAIN =
  "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1";
const APPEND_BOUNDARY_FINAL_NOTE_DOMAIN =
  "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1";

function source(relativePath) {
  return readFileSync(new URL(relativePath, `file://${REPO_ROOT}/`), "utf8");
}

function namesFromMatches(text, pattern) {
  return [...text.matchAll(pattern)].map((match) => match[1]);
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

function assertRunnerPrintsDotnetAndBridgeEvidence(script, envName, label) {
  const tmp = mkdtempSync(`${tmpdir()}/iroha-dotnet-runner-evidence-`);
  const fakeDotnet = `${tmp}/dotnet`;
  const fakeCargo = `${tmp}/cargo`;
  const bridgeTarget = `${tmp}/bridge-target`;
  try {
    writeFileSync(
      fakeDotnet,
      [
        "#!/usr/bin/env bash",
        "case \"${1:-}\" in",
        "  --version)",
        "    printf '%s\\n' '8.0.100'",
        "    exit 0",
        "    ;;",
        "  --info)",
        "    printf '%s\\n' '.NET SDK:'",
        "    printf '%s\\n' ' Version: 8.0.100'",
        "    printf '%s\\n' ' RID: fake-x64'",
        "    exit 0",
        "    ;;",
        "  test)",
        "    printf '%s\\n' \"fake dotnet test: $*\"",
        "    exit 0",
        "    ;;",
        "esac",
        "printf '%s\\n' \"unexpected fake dotnet invocation: $*\" >&2",
        "exit 64",
        "",
      ].join("\n"),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env bash",
        "if [[ \"${1:-}\" != 'build' || \"${2:-}\" != '-p' || \"${3:-}\" != 'connect_norito_bridge' ]]; then",
        "  printf '%s\\n' \"unexpected fake cargo invocation: $*\" >&2",
        "  exit 64",
        "fi",
        "mkdir -p \"${CARGO_TARGET_DIR}/debug\"",
        "printf '%s\\n' 'fake connect_norito_bridge' > \"${CARGO_TARGET_DIR}/debug/libconnect_norito_bridge.so\"",
        "printf '%s\\n' 'fake connect_norito_bridge' > \"${CARGO_TARGET_DIR}/debug/libconnect_norito_bridge.dylib\"",
        "printf '%s\\n' 'fake connect_norito_bridge' > \"${CARGO_TARGET_DIR}/debug/connect_norito_bridge.dll\"",
        "printf '%s\\n' 'fake cargo bridge build'",
        "exit 0",
        "",
      ].join("\n"),
    );
    chmodSync(fakeDotnet, 0o755);
    chmodSync(fakeCargo, 0o755);

    const result = spawnSync("bash", [script], {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: {
        ...process.env,
        PATH: `${tmp}:${process.env.PATH}`,
        [envName]: fakeDotnet,
        KAGEMUSHA_RECURSIVE_SPEND_CSHARP_BRIDGE_TARGET_DIR: bridgeTarget,
      },
    });

    assert.equal(result.status, 0, `${label} must succeed with fake .NET 8 and fake bridge build`);
    assert.match(result.stdout, /^8\.0\.100$/m, `${label} must print dotnet version evidence`);
    assert.match(result.stdout, /dotnet --info:\n\.NET SDK:\n Version: 8\.0\.100\n RID: fake-x64/u);
    assert.match(result.stdout, /fake cargo bridge build/u, `${label} must build the native bridge first`);
    assert.match(
      result.stdout,
      /connect_norito_bridge native bridge: .*connect_norito_bridge\.(?:dll|dylib|so)/u,
      `${label} must print the built native bridge path`,
    );
    assert.match(
      result.stdout,
      /connect_norito_bridge native bridge sha256: [0-9a-fA-F]{64}/u,
      `${label} must print the built native bridge digest`,
    );
    assert.match(result.stdout, /fake dotnet test: test /u, `${label} must invoke dotnet test`);
    assert.ok(
      result.stdout.indexOf("connect_norito_bridge native bridge sha256:") <
        result.stdout.indexOf("fake dotnet test: test "),
      `${label} must print native bridge evidence before running tests`,
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
  const modes = namesFromMatches(
    text.slice(start, end),
    /(--negative-control-[A-Za-z0-9-]+)/gu,
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
  const paths = namesFromMatches(
    text.slice(start, end),
    /"([^"]+)"/gu,
  ).filter(isPathInventoryString);
  assert.equal(new Set(paths).size, paths.length, `${startMarker} must not duplicate paths`);
  return [...paths].sort();
}

function quotedConstant(text, name) {
  const match = text.match(new RegExp(`${name} = "([^"]+)"`, "u"));
  assert.ok(match, `missing ${name}`);
  return match[1];
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

function assertContainsAll(text, names, label) {
  for (const name of names) {
    assert.ok(text.includes(name), `${label} missing ${name}`);
  }
}

function assertSameSet(actual, expected, label) {
  assert.deepEqual(
    [...actual].sort(),
    [...expected].sort(),
    `${label} drifted`,
  );
}

test("recursive Kagemusha ABI-6 C exports and shipped headers stay in parity", () => {
  const rustBridge = source("crates/connect_norito_bridge/src/lib.rs");
  const header = source("crates/connect_norito_bridge/include/connect_norito_bridge.h");
  const headerGuard = source("ci/check_connect_norito_bridge_header.sh");
  const umbrella = source("crates/connect_norito_bridge/include/NoritoBridge.h");

  const rustExports = new Set(
    namesFromMatches(
      rustBridge,
      /pub\s+unsafe\s+extern\s+"C"\s+fn\s+(connect_norito_kagemusha_recursive_spend_[a-z0-9_]+)\s*\(/gu,
    ),
  );
  const headerDeclarations = new Set(
    namesFromMatches(
      header,
      /int32_t\s+(connect_norito_kagemusha_recursive_spend_[a-z0-9_]+)\s*\(/gu,
    ),
  );

  assertSameSet(rustExports, REQUIRED_C_SYMBOLS, "Rust recursive Kagemusha C exports");
  assertSameSet(headerDeclarations, REQUIRED_C_SYMBOLS, "C header recursive Kagemusha declarations");
  assert.match(
    umbrella,
    /#include\s+"connect_norito_bridge\.h"/u,
    "NoritoBridge umbrella header must expose the bridge header",
  );
  assert.match(
    headerGuard,
    /expected_recursive_signatures[\s\S]*C header recursive-spend declaration has wrong signature/,
    "NoritoBridge header guard must reject recursive Kagemusha C signature drift",
  );
  assertContainsAll(
    headerGuard,
    REQUIRED_HEADER_NEGATIVE_CONTROL_MODES,
    "NoritoBridge header guard negative controls",
  );
});

test("recursive Kagemusha ABI-6 native host and SDK method names stay in parity", () => {
  assertContainsAll(
    source("crates/iroha_js_host/src/lib.rs"),
    REQUIRED_JS_NATIVE_METHODS.map((name) => `js_name = "${name}"`),
    "iroha_js_host NAPI exports",
  );
  assertContainsAll(
    source("javascript/iroha_js/src/crypto.js"),
    REQUIRED_JS_NATIVE_METHODS,
    "JavaScript source SDK",
  );
  assertContainsAll(
    source("javascript/iroha_js/dist/crypto.js"),
    REQUIRED_JS_NATIVE_METHODS,
    "JavaScript dist SDK",
  );
  assertContainsAll(
    source("javascript/iroha_js/src/crypto.browser.js"),
    REQUIRED_JS_NATIVE_METHODS,
    "JavaScript browser stubs",
  );
  assertContainsAll(
    source("javascript/iroha_js/dist/crypto.browser.js"),
    REQUIRED_JS_NATIVE_METHODS,
    "JavaScript dist browser stubs",
  );

  assertContainsAll(
    source("python/iroha_python/src/iroha_python/kagemusha.py"),
    REQUIRED_PYTHON_NATIVE_METHODS,
    "Python SDK",
  );
  assertContainsAll(
    source("python/iroha_python/iroha_python_rs/src/lib.rs"),
    REQUIRED_PYTHON_NATIVE_METHODS,
    "Python PyO3 host",
  );

  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"),
    REQUIRED_C_SYMBOLS,
    "Swift native bridge loader",
  );
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift"),
    [
      "initSpend",
      "appendSpend",
      "transitionProfileInit",
      "transitionProfileAppend",
      "lineageAppendBoundary",
      "lineageWitnessFromInitResult",
      "lineageWitnessAppendResult",
      "verifySpend",
      "redeemSpend",
    ],
    "Swift public prover",
  );

  assertContainsAll(
    source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"),
    [
      "initSpend",
      "appendSpend",
      "transitionProfileInit",
      "transitionProfileAppend",
      "lineageAppendBoundary",
      "lineageWitnessFromInitResult",
      "lineageWitnessAppendResult",
      "verifySpend",
      "redeemSpend",
      "nativeTransitionProfileInit",
      "nativeTransitionProfileAppend",
    ],
    "Android Java SDK",
  );
  assertContainsAll(
    source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"),
    [
      "initSpend",
      "appendSpend",
      "transitionProfileInit",
      "transitionProfileAppend",
      "lineageAppendBoundary",
      "lineageWitnessFromInitResult",
      "lineageWitnessAppendResult",
      "verifySpend",
      "redeemSpend",
      "nativeTransitionProfileInit",
      "nativeTransitionProfileAppend",
      "nativeBuildPallasOpenEnvelopesArchive",
      "nativeBuildPreviousProofOpenEnvelopesArchive",
    ],
    "Kotlin JVM SDK",
  );
  assertContainsAll(
    source("csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"),
    [
      "Init",
      "Append",
      "TransitionProfileInit",
      "TransitionProfileAppend",
      "LineageAppendBoundary",
      "LineageWitnessFromInitResult",
      "LineageWitnessAppendResult",
      "Verify",
      "Redeem",
      "ProveVerifiedCompactPaymentTokenWithRecords",
      "BuildPallasOpenEnvelopesArchive",
      "BuildPreviousProofOpenEnvelopesArchive",
      "IsPallasOpenEnvelopeBuilderAvailable",
      "ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
      "NativeTransitionProfileInit",
      "NativeTransitionProfileAppend",
      "NativeLineageAppendBoundary",
      "NativeCompactPaymentToken",
      "NativeBuildPallasOpenEnvelopesArchive",
      "NativeBuildPreviousProofOpenEnvelopesArchive",
      "NativeRecursiveAggregationProofBundle",
    ],
    "C# SDK",
  );
});

test("Kagemusha Kotlin recursive spend JNI declarations stay static", () => {
  const kotlinRecursive = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
  );
  const nativeMethods = [
    "nativeBridgeAbiVersion",
    "nativeInitSpend",
    "nativeAppendSpend",
    "nativeTransitionProfileInit",
    "nativeTransitionProfileAppend",
    "nativeLineageAppendBoundary",
    "nativeLineageWitnessFromInitResult",
    "nativeLineageWitnessAppendResult",
    "nativeVerifySpend",
    "nativeRedeemSpend",
    "nativeBuildPallasOpenEnvelopesArchive",
    "nativeBuildPreviousProofOpenEnvelopesArchive",
  ];

  for (const method of nativeMethods) {
    assert.match(
      kotlinRecursive,
      new RegExp(`@JvmStatic\\s*\\n\\s*private external fun ${method}\\s*\\(`, "u"),
      `Kotlin recursive spend ${method} must bind to the outer-class JNI symbol`,
    );
  }
});

test("Kagemusha mobile compact-token native output guards require Norito archives", () => {
  const javaCompact = source(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaCompactPaymentTokenProver.java",
  );
  const kotlinCompact = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCompactPaymentTokenProver.kt",
  );
  const javaRecursive = source(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
  );
  const kotlinRecursive = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
  );
  const javaRecursiveAggregation = source(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveAggregationProofBundleProver.java",
  );
  const kotlinRecursiveAggregation = source(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveAggregationProofBundleProver.kt",
  );
  const javaOfflineTests = source(
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineNoteTest.java",
  );
  const kotlinOfflineTests = source(
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteTest.kt",
  );
  const javaRecursiveTests = source(
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
  );
  const kotlinRecursiveTests = source(
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
  );

  for (const [label, text] of [
    ["Android Java compact-token prover", javaCompact],
    ["Kotlin compact-token prover", kotlinCompact],
  ]) {
    assert.match(text, /NORITO_MAGIC/u, `${label} must check Norito magic`);
    assert.match(text, /CRC64_REFLECTED_POLY/u, `${label} must carry the Norito CRC64 profile`);
    assert.match(text, /isValidNoritoArchive/u, `${label} must validate Norito archive structure`);
    assert.match(text, /hasNonEmptyNoritoPayload/u, `${label} must require a non-empty Norito payload`);
    assert.match(text, /requireNativeInput/u, `${label} must preflight compact-token native input archives`);
    assert.match(text, /must be a valid Norito archive/u, `${label} must reject malformed native inputs`);
    assert.match(
      text,
      /must contain a non-empty Norito payload/u,
      `${label} must reject empty-payload native inputs`,
    );
    assert.match(text, /returned invalid Norito archive/u, `${label} must reject malformed native output`);
    assert.match(text, /returned empty Norito payload/u, `${label} must reject empty-payload native output`);
  }

  for (const [label, text] of [
    ["Android Java recursive spend prover", javaRecursive],
    ["Kotlin recursive spend prover", kotlinRecursive],
  ]) {
    assert.match(
      text,
      /KagemushaCompactPaymentTokenProver\.isValidNoritoArchive/u,
      `${label} must reuse the shared Norito archive validator`,
    );
    assert.match(
      text,
      /KagemushaCompactPaymentTokenProver\.hasNonEmptyNoritoPayload/u,
      `${label} must require non-empty Norito native output payloads`,
    );
    assert.match(text, /requireNativeInput/u, `${label} must preflight native input archives`);
    assert.match(text, /must be a valid Norito archive/u, `${label} must reject malformed native inputs`);
    assert.match(
      text,
      /must contain a non-empty Norito payload/u,
      `${label} must reject empty-payload native inputs`,
    );
    assert.match(text, /returned invalid Norito archive/u, `${label} must reject malformed native output`);
    assert.match(text, /returned empty Norito payload/u, `${label} must reject empty-payload native output`);
  }

  for (const [label, text] of [
    ["Android Java recursive aggregation prover", javaRecursiveAggregation],
    ["Kotlin recursive aggregation prover", kotlinRecursiveAggregation],
  ]) {
    assert.match(
      text,
      /KagemushaCompactPaymentTokenProver\.requireNativeInput/u,
      `${label} must reuse the shared Norito input validator`,
    );
    assert.match(text, /recordBundleArchive/u, `${label} must validate record-bundle inputs`);
    assert.match(text, /pallasOpenEnvelopesArchive/u, `${label} must validate Pallas opening inputs`);
  }

  for (const [label, text] of [
    ["Android Java offline tests", javaOfflineTests],
    ["Kotlin offline tests", kotlinOfflineTests],
  ]) {
    assert.match(
      text,
      /kagemushaNativeProversRejectMissingAndEmptyNativeOutputs[\s\S]*returned invalid Norito archive/u,
      `${label} must test malformed compact-token native output rejection`,
    );
    assert.match(
      text,
      /kagemushaNativeProversRejectMissingAndEmptyNativeOutputs[\s\S]*returned empty Norito payload/u,
      `${label} must test empty-payload compact-token native output rejection`,
    );
    assert.match(
      text,
      /kagemushaNoritoFrameWithPayload/u,
      `${label} must retain a deterministic valid Norito output fixture`,
    );
    assert.match(
      text,
      /kagemushaRecordBackedNativeProverValidatesInput[\s\S]*recordBundleArchive must be a valid Norito archive/u,
      `${label} must test malformed compact-token native input rejection`,
    );
    assert.match(
      text,
      /kagemushaRecordBackedNativeProverValidatesInput[\s\S]*recordBundleArchive must contain a non-empty Norito payload/u,
      `${label} must test empty-payload compact-token native input rejection`,
    );
    assert.match(
      text,
      /kagemushaRecursiveAggregationNativeProverValidatesInput[\s\S]*pallasOpenEnvelopesArchive must be a valid Norito archive/u,
      `${label} must test malformed recursive aggregation Pallas input rejection`,
    );
    assert.match(
      text,
      /kagemushaRecursiveAggregationNativeProverValidatesInput[\s\S]*pallasOpenEnvelopesArchive must contain a non-empty Norito payload/u,
      `${label} must test empty-payload recursive aggregation Pallas input rejection`,
    );
  }

  for (const [label, text] of [
    ["Android Java recursive spend tests", javaRecursiveTests],
    ["Kotlin recursive spend tests", kotlinRecursiveTests],
  ]) {
    assert.match(
      text,
      /rejectsNullAndEmptyNativeRedeemOutput[\s\S]*returned invalid Norito archive/u,
      `${label} must test malformed recursive native output rejection`,
    );
    assert.match(
      text,
      /rejectsNullAndEmptyNativeRedeemOutput[\s\S]*returned empty Norito payload/u,
      `${label} must test empty-payload recursive native output rejection`,
    );
    assert.match(
      text,
      /assertRejectsMalformedNativeRedeemOutput/u,
      `${label} must route malformed recursive native output variants through a shared assertion`,
    );
    assert.match(
      text,
      /compressed\[22\] = 1/u,
      `${label} must reject compressed recursive native output frames`,
    );
    assert.match(
      text,
      /unsupportedFlags\[39\] = 0x08/u,
      `${label} must reject unsupported recursive native output flags`,
    );
    assert.match(
      text,
      /invalidFieldBitset\[39\] = 0x20/u,
      `${label} must reject invalid field-bitset recursive native output flags`,
    );
    assert.match(
      text,
      /withHeaderPadding\(kagemushaNoritoFrameWithPayload\(0x4b\),\s*(?:byteArrayOf\(0x7f\)|new byte\[\] \{0x7f\})\)/u,
      `${label} must reject nonzero recursive native output header padding`,
    );
    assert.match(
      text,
      /withHeaderPadding\(kagemushaNoritoFrameWithPayload\(0x4b\),\s*(?:ByteArray\(65\)|new byte\[65\])\)/u,
      `${label} must reject excessive recursive native output header padding`,
    );
    assert.match(
      text,
      /kagemushaNoritoFrameWithPayload/u,
      `${label} must retain a deterministic valid Norito output fixture`,
    );
    assert.match(
      text,
      /rejectsMalformedAndEmptyPayloadArchivesBeforeNativeDispatch[\s\S]*requestArchive must be a valid Norito archive/u,
      `${label} must test malformed recursive native input rejection`,
    );
    assert.match(
      text,
      /rejectsMalformedAndEmptyPayloadArchivesBeforeNativeDispatch[\s\S]*previousWitnessArchive must contain a non-empty Norito payload/u,
      `${label} must test empty-payload recursive native input rejection`,
    );
  }
});

test("Kagemusha mobile offline-note proof metadata rejects padded selectors", () => {
  for (const [relative, label] of [
    [
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNote.java",
      "Android Java Offline Note proof metadata",
    ],
    [
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteV2.java",
      "Android Java Offline Note V2 proof metadata",
    ],
    [
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNote.kt",
      "Kotlin Offline Note proof metadata",
    ],
    [
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNoteV2.kt",
      "Kotlin Offline Note V2 proof metadata",
    ],
  ]) {
    assertContainsAll(
      source(relative),
      [
        "requireNonBlankUnpadded",
        "verifying key backend",
        "verifying key name",
        "proof backend",
        "must not contain surrounding whitespace",
      ],
      `${label} exactness`,
    );
  }

  for (const [relative, label] of [
    [
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/VerifyingKeyBoxCodec.java",
      "Android Java VerifyingKeyBox metadata",
    ],
    [
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/VerifyingKeyBoxCodec.kt",
      "Kotlin VerifyingKeyBox metadata",
    ],
  ]) {
    assertContainsAll(
      source(relative),
      [
        'requireNonBlankUnpadded(backend, "backend")',
        "decodeNorito",
        "Trailing bytes after VerifyingKeyBox field decode",
        "must not contain surrounding whitespace",
        "bytes must not be empty",
      ],
      `${label} exactness`,
    );
  }

  assertContainsAll(
    source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineNoteTest.java"),
    [
      "recursiveProofMetadataRejectsPaddedAndMalformedVerifierKeys",
      "padded verifier backend must be rejected",
      "padded verifier name must be rejected",
      "padded proof backend must be rejected",
      'VerifyingKeyBoxCodec.encodeNorito(" halo2/ipa "',
      "verifyingKeyBoxStandaloneCodecDecodesAndRejectsMalformedArchives",
      "rawVerifyingKeyBoxNorito",
      "Trailing bytes after VerifyingKeyBox field decode",
      "padded verifying key backend should fail",
    ],
    "Android Java proof metadata exactness tests",
  );
  assertContainsAll(
    source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineNoteV2Test.java"),
    [
      "padded proof backend should throw",
      'new OfflineNoteV2.VerifyingKeyIdReference(" halo2/ipa ", "vk")',
      'new OfflineNoteV2.VerifyingKeyIdReference("halo2/ipa", " vk ")',
      "padded verifier name should throw",
    ],
    "Android Java Offline Note V2 proof metadata exactness tests",
  );
  assertContainsAll(
    source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteTest.kt"),
    [
      "recursiveProofMetadataRejectsPaddedAndMalformedVerifierKeys",
      "verifyingKeyBoxStandaloneCodecDecodesAndRejectsMalformedArchives",
      "rawVerifyingKeyBoxNorito",
      "Trailing bytes after VerifyingKeyBox field decode",
      'OfflineNote.VerifyingKeyBox(" halo2/ipa ", byteArrayOf(1))',
      '"  ${OfflineNote.RECURSIVE_BACKEND}  "',
      '"  ${OfflineNote.RECURSIVE_VERIFIER_NAME}  "',
    ],
    "Kotlin proof metadata exactness tests",
  );
  assertContainsAll(
    source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteV2Test.kt"),
    [
      'OfflineNoteV2.ProofBox("  ${OfflineNoteV2.RECURSIVE_BACKEND}  ", byteArrayOf(1))',
      'OfflineNoteV2.VerifyingKeyIdReference(backend = " halo2/ipa ", name = "vk")',
      'OfflineNoteV2.VerifyingKeyIdReference(backend = "halo2/ipa", name = " vk ")',
    ],
    "Kotlin Offline Note V2 proof metadata exactness tests",
  );

  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/TxBuilder.swift"),
    [
      "case surroundingWhitespace",
      "trimmingCharacters(in: .whitespacesAndNewlines) == backend",
      "trimmingCharacters(in: .whitespacesAndNewlines) == name",
      "Verifying key backend and name must not contain surrounding whitespace.",
    ],
    "Swift verifier-key id exactness",
  );
  for (const [relative, label] of [
    ["IrohaSwift/Sources/IrohaSwift/OfflineNote.swift", "Swift Offline Note proof metadata"],
    ["IrohaSwift/Sources/IrohaSwift/OfflineNoteV2.swift", "Swift Offline Note V2 proof metadata"],
  ]) {
    assertContainsAll(
      source(relative),
      [
        "let trimmedBackend = backend.trimmingCharacters(in: .whitespacesAndNewlines)",
        "guard trimmedBackend == backend else",
        "unsupportedRecursiveProofBackend",
        "self.backend = backend",
      ],
      `${label} exactness`,
    );
  }
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/TxBuilderTests.swift"),
    [
      "testVerifyingKeyIdReferenceValidation",
      'VerifyingKeyIdReference(backend: " halo2/ipa ", name: "vk")',
      'VerifyingKeyIdReference(backend: "halo2/ipa", name: " vk ")',
      ".surroundingWhitespace",
    ],
    "Swift verifier-key id exactness tests",
  );
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteTests.swift"),
    [
      "testOfflineNoteProofAndHashValidationRejectsMalformedValues",
      "testOfflineNoteRecursiveProofCoversCustomVerifierAndVerifierValidation",
      'proofBackend: " custom_proof_backend "',
      'verifierBackend: " custom_backend "',
      'verifierName: " custom_vk "',
      ".surroundingWhitespace",
    ],
    "Swift Offline Note proof metadata exactness tests",
  );
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteV2Tests.swift"),
    [
      "testOfflineNoteV2ProofAndHashValidationRejectsMalformedValues",
      "testOfflineNoteV2RecursiveProofCoversCustomVerifierAndVerifierValidation",
      'proofBackend: " custom_proof_backend "',
      'verifierBackend: " custom_backend "',
      'verifierName: " custom_vk "',
      ".surroundingWhitespace",
    ],
    "Swift Offline Note V2 proof metadata exactness tests",
  );
});

test("Kagemusha mobile Offline Note V2 OpenVerifyEnvelope decoders stay wired", () => {
  for (const [relative, label] of [
    [
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteV2Halo2Prover.java",
      "Android Java Offline Note V2 Halo2 prover",
    ],
    [
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNoteV2Halo2Prover.java",
      "Kotlin Offline Note V2 Halo2 prover",
    ],
  ]) {
    const text = source(relative);
    assertContainsAll(
      text,
      [
        "verifyOpenVerifyEnvelope",
        "proofPayloadFromOpenVerifyEnvelope",
        "readOpenVerifyEnvelopePayload",
        "Trailing bytes after OpenVerifyEnvelope field decode",
        "OpenVerifyEnvelope proof payload is empty",
      ],
      `${label} OpenVerifyEnvelope decode`,
    );
    assert.ok(
      !text.includes("OpenVerifyEnvelope decoding is not supported"),
      `${label} must not keep the OpenVerifyEnvelope decode stub`,
    );
  }

  for (const [relative, label] of [
    [
      "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineNoteV2Test.java",
      "Android Java Offline Note V2 tests",
    ],
    [
      "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/OfflineNoteV2Test.kt",
      "Kotlin Offline Note V2 tests",
    ],
  ]) {
    assertContainsAll(
      source(relative),
      [
        "openVerifyEnvelopeDecoderRejectsMalformedV2EnvelopeFields",
        "rawOpenVerifyEnvelopeWithCircuitPayload",
        "Trailing bytes after OpenVerifyEnvelope field decode",
        "OpenVerifyEnvelope proof payload is empty",
      ],
      `${label} OpenVerifyEnvelope decode regressions`,
    );
  }
});

test("Kagemusha JavaScript and Python native output guards require Norito archives", () => {
  for (const relative of ["javascript/iroha_js/src/crypto.js", "javascript/iroha_js/dist/crypto.js"]) {
    assertContainsAll(
      source(relative),
      [
        "kagemushaRecursiveSpendOutputToBuffer",
        "assertKagemushaNoritoArchive(",
        "native ${operation} returned invalid Norito archive",
        "native ${operation} returned empty Norito payload",
      ],
      `${relative} native output guard`,
    );
  }
  assertContainsAll(
    source("javascript/iroha_js/test/kagemushaRecursiveSpend.test.js"),
    [
      "Kagemusha recursive spend helpers reject malformed Norito native outputs",
      "Kagemusha recursive spend helpers reject empty-payload Norito native outputs",
      "native kagemushaRecursiveSpendRedeem returned invalid Norito archive",
      "native kagemushaRecursiveSpendRedeem returned empty Norito payload",
      "assertRejectsMalformedNativeRedeemOutput",
      "compressed[22] = 1",
      "unsupportedFlags[39] = 0x08",
      "invalidFieldBitset[39] = 0x20",
      "kagemushaNoritoFrameWithHeaderPadding",
      "Buffer.from([0x7f])",
      "Buffer.alloc(65)",
      "kagemushaNoritoFrameWithPayload",
    ],
    "JavaScript native output guard tests",
  );

  assertContainsAll(
    source("python/iroha_python/src/iroha_python/kagemusha.py"),
    [
      "_require_kagemusha_native_output",
      "_assert_kagemusha_norito_archive(output, name)",
      "returned invalid Norito archive",
      "returned empty Norito payload",
    ],
    "Python native output guard",
  );
  assertContainsAll(
    source("python/iroha_python/tests/kagemusha_test.py"),
    [
      "test_recursive_kagemusha_helpers_reject_malformed_native_outputs",
      "test_recursive_kagemusha_helpers_reject_empty_payload_native_outputs",
      "returned invalid Norito archive",
      "returned empty Norito payload",
      "assert_rejects_malformed_native_outputs",
      "compressed[22] = 1",
      "unsupported_flags[39] = 0x08",
      "invalid_field_bitset[39] = 0x20",
      "_kagemusha_norito_frame_with_header_padding",
      'b"\\x7f"',
      'b"\\x00" * 65',
      "_kagemusha_norito_frame_with_payload",
    ],
    "Python native output guard tests",
  );
});

test("Kagemusha JavaScript and Python recursive spend inputs require Norito archives", () => {
  for (const relative of ["javascript/iroha_js/src/crypto.js", "javascript/iroha_js/dist/crypto.js"]) {
    assertContainsAll(
      source(relative),
      [
        "toKagemushaArchiveView(value, name)",
        "toOwnedKagemushaArchiveBuffer(value, name)",
        "view.length > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "const request = toOwnedKagemushaArchiveBuffer(requestArchive, archiveName)",
        'const recordBundle = toOwnedKagemushaArchiveBuffer(',
        'const compactToken = toOwnedKagemushaArchiveBuffer(',
        "outputView.length > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "const output = Buffer.from(outputView)",
        "assertKagemushaNoritoArchive(",
        "previousWitnessArchive",
        "compactTokenArchive",
      ],
      `${relative} recursive spend input guard`,
    );
  }
  assertContainsAll(
    source("javascript/iroha_js/test/kagemushaRecursiveSpend.test.js"),
    [
      "Kagemusha recursive spend helpers reject oversized request archives before native calls",
      "requestArchive must not exceed",
      "recordBundleArchive must not exceed",
      "pallasOpenEnvelopesArchive must not exceed",
      "previousWitnessArchive must not exceed",
      "compactTokenArchive must not exceed",
      "Kagemusha recursive spend helpers reject malformed Norito request archives before native calls",
      "Kagemusha recursive spend helpers reject empty-payload Norito request archives before native calls",
      "requestArchive must be a valid Norito archive",
      "recordBundleArchive must be a valid Norito archive",
      "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
      "previousWitnessArchive must contain a non-empty Norito payload",
      "kagemushaInputArchive",
    ],
    "JavaScript recursive spend input guard tests",
  );

  assertContainsAll(
      source("python/iroha_python/src/iroha_python/kagemusha.py"),
      [
        "_archive_bytes_named",
        "_norito_archive_bytes_named",
        "view = memoryview(archive)",
        "view.nbytes > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        "return view.tobytes()",
        "view = memoryview(result)",
        "output = view.tobytes()",
        "_assert_kagemusha_norito_archive(data, name)",
        '_norito_archive_bytes_named(record_bundle_archive, "record_bundle_archive")',
        '_norito_archive_bytes_named(pallas_open_envelopes_archive, "pallas_open_envelopes_archive")',
        '_norito_archive_bytes_named(request_archive, "request_archive")',
        '_norito_archive_bytes_named(bundle_archive, "bundle_archive")',
      ],
    "Python recursive spend input guard",
  );
  assertContainsAll(
    source("python/iroha_python/tests/kagemusha_test.py"),
    [
      "test_recursive_kagemusha_helpers_reject_oversized_inputs_before_copy_and_native",
      "oversized Kagemusha input reached native loading",
      "test_recursive_kagemusha_helpers_reject_oversized_memoryview_native_outputs",
      "must not exceed",
      "compact_token_archive",
      "test_recursive_kagemusha_helpers_reject_malformed_norito_requests",
      "test_recursive_kagemusha_helpers_reject_empty_payload_norito_requests",
      "test_kagemusha_native_prover_helpers_reject_malformed_norito_requests",
      "test_kagemusha_native_prover_helpers_reject_empty_payload_norito_requests",
      "record_bundle_archive must be a valid Norito archive",
      "pallas_open_envelopes_archive must contain a non-empty Norito payload",
      "request_archive must be a valid Norito archive",
      "previous_witness_archive must contain a non-empty Norito payload",
      "_kagemusha_input_archive",
    ],
    "Python recursive spend input guard tests",
  );
});

test("Kagemusha Swift and C# recursive spend inputs require Norito archives", () => {
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift"),
    [
      "invalidInputArchive",
      "emptyInputPayload",
      "try archives.forEach(requireValidInputArchive)",
      "noritoDecodeFrame(archive)",
      "Kagemusha recursive spend input archive must be a valid Norito archive.",
      "Kagemusha recursive spend input archive must contain a non-empty Norito payload.",
    ],
    "Swift recursive spend input guard",
  );
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift"),
    [
      "testRejectsMalformedInputArchivesBeforeBridgeCall",
      "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
      ".invalidInputArchive",
      ".emptyInputPayload",
      "validKagemushaNoritoArchive",
      "emptyPayloadKagemushaNoritoArchive",
    ],
    "Swift recursive spend input guard tests",
  );
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift"),
    [
      "invalidRecordBundleArchive",
      "emptyRecordBundlePayload",
        "invalidPallasOpenEnvelopesArchive",
        "emptyPallasOpenEnvelopesPayload",
        "invalidKeyArtifactsArchive",
        "emptyKeyArtifactsPayload",
        "invalidVerifierKeysArchive",
        "emptyVerifierKeysPayload",
        "invalidBundleArchive",
      "emptyBundlePayload",
      "try requireValidInputArchive(",
      "Kagemusha verified fold record bundle archive must be a valid Norito archive.",
      "Kagemusha Pallas open-envelope archive must contain a non-empty Norito payload.",
      "Kagemusha recursive spend bundle archive must be a valid Norito archive.",
      "Kagemusha recursive spend bundle archive must contain a non-empty Norito payload.",
    ],
    "Swift recursive compact prover input guard",
  );
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift"),
    [
      "testRejectsMalformedInputArchivesBeforeBridgeCall",
      "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
      "testRejectsInvalidKeyArtifactsArchiveBeforeBridgeCall",
      "testVerifyRejectsInvalidVerifierKeysArchiveBeforeBridgeCall",
      ".invalidRecordBundleArchive",
      ".emptyPallasOpenEnvelopesPayload",
      ".invalidKeyArtifactsArchive",
      ".emptyVerifierKeysPayload",
      ".invalidBundleArchive",
      "testProjectionRejectsMalformedBundleArchiveBeforeBridgeCall",
    ],
    "Swift recursive compact prover input guard tests",
  );
  assertContainsAll(
    source("csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"),
    [
      "RequireValidInputArchive",
      "Request archive",
      "Bundle archive",
      "Record bundle archive",
      "Pallas open-envelopes archive",
      "ProveVerifiedCompactPaymentTokenWithRecords",
      "ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
      "RecursiveSpendCompactPaymentTokenFromBundle",
      "must be a valid Norito archive.",
      "must contain a non-empty Norito payload.",
      "PrivacyNative.IsNoritoV1Archive(bytes)",
      "PrivacyNative.HasNonEmptyPrivacyNoritoPayload(bytes)",
    ],
    "C# recursive spend input guard",
  );
  assertContainsAll(
    source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs"),
    [
      "RecursiveSpendNativeRejectsMalformedArchivesBeforeLoadingNativeBridge",
      "RecursiveSpendNativeRejectsEmptyPayloadArchivesBeforeLoadingNativeBridge",
      "CompactTokenProverRejectsMalformedInputsBeforeLoadingNativeBridge",
      "CompactTokenProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
      "RecursiveAggregationProverRejectsMalformedInputsBeforeLoadingNativeBridge",
      "RecursiveAggregationProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
      "RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge",
      "RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
      "RecursiveSpendCompactProjectionRejectsInvalidBundleBeforeLoadingNativeBridge",
      "Record bundle archive must be a valid Norito archive",
      "Recursive spend bundle archive must be a valid Norito archive",
      "Pallas open-envelopes archive must contain a non-empty Norito payload",
      "KagemushaNoritoFrameWithPayload",
      "AssertRejectsMalformedEverywhere",
      "AssertRejectsMalformedEverywhere(compressed, validArchive)",
      "AssertRejectsMalformedEverywhere(unsupportedFlags, validArchive)",
      "AssertRejectsMalformedEverywhere(invalidFieldBitset, validArchive)",
      "compressed[22] = 1",
      "unsupportedFlags[39] = 0x08",
      "invalidFieldBitset[39] = 0x20",
      "WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[] { 0x7f })",
      "WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[65])",
    ],
    "C# recursive spend input guard tests",
  );
});

test("Kagemusha JVM and Android recursive compact prover inputs require Norito archives", () => {
  for (const [relative, label] of [
    [
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt",
      "Kotlin recursive compact prover input guard",
    ],
    [
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
      "Android recursive compact prover input guard",
    ],
  ]) {
    assertContainsAll(
      source(relative),
      [
        "requireNativeInput",
        'requireNativeInput(recordBundleArchive, "recordBundleArchive")',
        'requireNativeInput(pallasOpenEnvelopesArchive, "pallasOpenEnvelopesArchive")',
        'recursiveCompactKeyArtifactsArchive, "recursiveCompactKeyArtifactsArchive"',
        'ownedNativeInput(bundleArchive, "bundleArchive")',
        "KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)",
        "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)",
        "must be a valid Norito archive",
        "must contain a non-empty Norito payload",
      ],
      label,
    );
  }
  for (const [relative, label] of [
    [
      "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
      "Kotlin recursive compact prover input guard tests",
    ],
    [
      "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
      "Android recursive compact prover input guard tests",
    ],
  ]) {
    assertContainsAll(
      source(relative),
      [
        "validRecursiveCompactInput",
        "proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes",
        "recordBundleArchive must not be empty",
        "pallasOpenEnvelopesArchive must not be empty",
        "recursiveCompactKeyArtifactsArchive must not be empty",
        "recordBundleArchive must be a valid Norito archive",
        "pallasOpenEnvelopesArchive must be a valid Norito archive",
        "recursiveCompactKeyArtifactsArchive must be a valid Norito archive",
        "bundleArchive must be a valid Norito archive",
        "recordBundleArchive must contain a non-empty Norito payload",
        "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
        "recursiveCompactVerifierKeysArchive must be a valid Norito archive",
        "bundleArchive must contain a non-empty Norito payload",
        "kagemushaNoritoFrameWithPayload",
      ],
      label,
    );
  }
});

test("Kagemusha Swift compact-token prover inputs and outputs require Norito archives", () => {
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaCompactPaymentTokenProver.swift"),
    [
      "invalidRecordBundleArchive",
      "emptyRecordBundlePayload",
      "oversizedCompactTokenArchive",
      "invalidCompactTokenArchive",
      "emptyCompactTokenPayload",
      "try requireValidRecordBundleArchive(recordBundleArchive)",
      "try requireValidCompactTokenArchive(token)",
      "noritoDecodeFrame(archive)",
      "Kagemusha verified fold record bundle archive must be a valid Norito archive.",
      "Kagemusha verified fold record bundle archive must contain a non-empty Norito payload.",
      "Kagemusha compact-token native bridge returned an invalid Norito archive.",
      "Kagemusha compact-token native bridge returned an empty Norito payload.",
    ],
    "Swift compact-token input/output guard",
  );
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift"),
    [
      "testRejectsMalformedRecordBundleArchiveBeforeBridgeCall",
      "testRejectsEmptyPayloadRecordBundleArchiveBeforeBridgeCall",
      "testRejectsMalformedNativeOutput",
      "testRejectsEmptyPayloadNativeOutput",
      "testReturnsValidNativeOutput",
      ".invalidRecordBundleArchive",
      ".emptyRecordBundlePayload",
      ".invalidCompactTokenArchive",
      ".emptyCompactTokenPayload",
      "validKagemushaNoritoArchive",
      "emptyPayloadKagemushaNoritoArchive",
      "malformedKagemushaNoritoArchives",
      "compressed[22] = 0x01",
      "unsupportedFlags[39] = NoritoHeader.varintOffsets",
      "invalidFieldBitset[39] = NoritoHeader.fieldBitset",
      "kagemushaNoritoFrameWithHeaderPadding",
      "Data([0x7f])",
      "Data(repeating: 0, count: 65)",
    ],
    "Swift compact-token input/output guard tests",
  );
});

test("Kagemusha Swift recursive aggregation prover inputs and outputs require Norito archives", () => {
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveAggregationProofBundleProver.swift"),
    [
      "invalidRecordBundleArchive",
      "emptyRecordBundlePayload",
      "invalidPallasOpenEnvelopesArchive",
      "emptyPallasOpenEnvelopesPayload",
      "oversizedProofBundleArchive",
      "invalidProofBundleArchive",
      "emptyProofBundlePayload",
      "try requireValidInputArchive(",
      "try requireValidProofBundleArchive(proofBundle)",
      "noritoDecodeFrame(archive)",
      "Kagemusha verified fold record bundle archive must be a valid Norito archive.",
      "Kagemusha Pallas open-envelope archive must contain a non-empty Norito payload.",
      "Kagemusha recursive aggregation native bridge returned an invalid Norito archive.",
      "Kagemusha recursive aggregation native bridge returned an empty Norito payload.",
    ],
    "Swift recursive aggregation input/output guard",
  );
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift"),
    [
      "testRejectsMalformedInputArchivesBeforeBridgeCall",
      "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
      "testRejectsMalformedNativeOutput",
      "testRejectsEmptyPayloadNativeOutput",
      "testReturnsValidNativeOutput",
      ".invalidRecordBundleArchive",
      ".emptyPallasOpenEnvelopesPayload",
      ".invalidProofBundleArchive",
      ".emptyProofBundlePayload",
      "validKagemushaNoritoArchive",
      "emptyPayloadKagemushaNoritoArchive",
      "malformedKagemushaNoritoArchives",
      "compressed[22] = 0x01",
      "unsupportedFlags[39] = NoritoHeader.varintOffsets",
      "invalidFieldBitset[39] = NoritoHeader.fieldBitset",
      "kagemushaNoritoFrameWithHeaderPadding",
      "Data([0x7f])",
      "Data(repeating: 0, count: 65)",
    ],
    "Swift recursive aggregation input/output guard tests",
  );
});

test("Kagemusha Swift recursive spend native outputs require Norito archives", () => {
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift"),
    [
      "invalidNativeOutput",
      "emptyNativeOutputPayload",
      "try requireValidOutputArchive(archive)",
      "Kagemusha recursive spend native bridge returned an invalid Norito archive.",
      "Kagemusha recursive spend native bridge returned an empty Norito payload.",
    ],
    "Swift recursive spend output guard",
  );
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift"),
    [
      "testRejectsMalformedNativeOutput",
      "testRejectsEmptyPayloadNativeOutput",
      "testReturnsValidNativeOutput",
      ".invalidNativeOutput",
      ".emptyNativeOutputPayload",
      "validKagemushaNoritoArchive",
      "emptyPayloadKagemushaNoritoArchive",
      "malformedKagemushaNoritoArchives",
      "compressed[22] = 0x01",
      "unsupportedFlags[39] = NoritoHeader.varintOffsets",
      "invalidFieldBitset[39] = NoritoHeader.fieldBitset",
      "kagemushaNoritoFrameWithHeaderPadding",
      "Data([0x7f])",
      "Data(repeating: 0, count: 65)",
    ],
    "Swift recursive spend output guard tests",
  );
});

test("Kagemusha Swift native bridge caps Kagemusha outputs before Data copies", () => {
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"),
    [
      "copyKagemushaNativeArchiveOutput",
      "length <= CUnsignedLong(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes)",
      "throw NativeBridgeError.kagemushaProve",
      "return try Self.copyKagemushaNativeArchiveOutput(",
    ],
    "Swift native bridge Kagemusha output cap",
  );
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift"),
    [
      "testNativeBridgeCopiesBoundedKagemushaOutputAndFreesNativePointer",
      "testNativeBridgeRejectsOversizedKagemushaOutputBeforeCopying",
      "KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1",
      "XCTAssertTrue(didFree)",
    ],
    "Swift native bridge Kagemusha output cap tests",
  );
});

test("Kagemusha Swift instruction transaction builder stays wired", () => {
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaInstructionTransactionEncoder.swift"),
    [
      "public enum KagemushaInstructionTransactionError",
      "public enum KagemushaInstructionType",
      'case transfer = "KagemushaTransfer"',
      'case redeemRecursive = "RedeemKagemushaRecursive"',
      "public struct KagemushaInstructionTransactionRequest",
      "public struct KagemushaRecursiveRedeemTransactionRequest",
      "public enum KagemushaRecursiveRedeemRequestArchive",
      "KagemushaRecursiveRedeemRequestArchiveError",
      "unexpectedInstructionArchiveType(expected: KagemushaInstructionType, actual: KagemushaInstructionType)",
      "KagemushaRecursiveSpendRedeemRequestV1",
      "static func encodeKagemushaInstruction(",
      "static func encodeKagemushaRecursiveRedeem(",
      "func buildKagemushaInstruction(",
      "func buildKagemushaRecursiveRedeem(",
      "try KagemushaRecursiveSpendProver.redeemSpend(requestArchive: $0)",
      "KagemushaRecursiveCompactPaymentTokenProver.nativeArchiveMaxBytes",
      "frame.header.compression == .none",
      "frame.header.schema == noritoSchemaHash(forTypeName: type.wireName)",
      "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1",
      "noritoSchemaHash(forTypeName: schemaName) == frame.header.schema",
    ],
    "Swift Kagemusha instruction transaction builder",
  );
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/KagemushaInstructionTransactionEncoderTests.swift"),
    [
      "testBuildRecursiveRedeemInstructionTransactionWrapsNativeInstructionArchive",
      "testBuildKagemushaTransferInstructionTransactionUsesTransferWireName",
      "testKagemushaArchiveValidationAcceptsSharedAbi7Fixtures",
      "testBuildKagemushaRecursiveRedeemTransactionDerivesInstructionBeforeSigning",
      "testKagemushaInstructionTransactionRejectsAdversarialArchives",
      "testKagemushaRecursiveRedeemTransactionRejectsMalformedRequestBeforeNativeRedeem",
      "testKagemushaRecursiveRedeemTransactionRejectsAdversarialNativeInstructionArchives",
      "testKagemushaRecursiveRedeemRequestArchiveValidationRejectsAdversarialInputs",
      "testKagemushaInstructionRequestValidationRejectsInvalidInputsBeforeSigning",
      ".unsupportedInstructionArchiveType",
      ".unsupportedRequestArchiveType",
      ".unexpectedInstructionArchiveType(expected: .redeemRecursive, actual: .transfer)",
      "unsupportedFlagsArchive",
      "invalidFieldBitsetArchive",
      "nonZeroPaddingArchive",
      "excessivePaddingArchive",
      "KagemushaRecursiveCompactPaymentTokenProver.nativeArchiveMaxBytes + 1",
    ],
    "Swift Kagemusha instruction transaction builder tests",
  );
});

test("Kagemusha C# instruction transaction builder stays wired", () => {
  assertContainsAll(
    source("csharp/src/Hyperledger.Iroha.Sdk/Transactions/KagemushaInstructionArchiveInstruction.cs"),
    [
      "public enum KagemushaInstructionType",
      "RedeemRecursive",
      "ArchiveTypeName",
      "WireName",
      '"KagemushaTransfer"',
      '"RedeemKagemushaRecursive"',
      '"iroha_data_model::isi::offline::KagemushaTransfer"',
      '"iroha_data_model::isi::offline::RedeemKagemushaRecursive"',
      "CopyAndValidateArchive",
      "KagemushaRecursiveSpendNative.NativeArchiveMaxBytes",
      "PrivacyNative.IsNoritoV1Archive(copy)",
      "PrivacyNative.HasNonEmptyPrivacyNoritoPayload(copy)",
      "NoritoCodec.SchemaHash(instructionType.WireName())",
      "SequenceEqual(expectedSchema)",
      "KagemushaRecursiveSpendRedeemInstructionArchive",
      "EncodeFramedPayload",
      "return InstructionArchive;",
    ],
    "C# Kagemusha instruction archive transaction instruction",
  );
  assertContainsAll(
    source("csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionInstruction.cs"),
    [
      "internal virtual byte[] EncodeFramedPayload",
      "NoritoCodec.Encode(TypeName, EncodePayload(context))",
      "KagemushaInstructionArchive(",
      "KagemushaRecursiveRedeem(",
      "KagemushaInstructionArchiveInstruction.RedeemRecursive",
    ],
    "C# Kagemusha instruction transaction factories",
  );
  assertContainsAll(
    source("csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionEncodingContext.cs"),
    [
      "instruction.EncodeFramedPayload(this)",
      "writer.WriteField(EncodeString(instruction.WireId))",
      "writer.WriteField(EncodeBytesVec(framedInstruction))",
    ],
    "C# Kagemusha instruction archive pass-through encoder",
  );
  assertContainsAll(
    source("csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionBuilder.cs"),
    [
      "KagemushaInstructionArchive(",
      "KagemushaRecursiveRedeem(",
      "KagemushaRecursiveSpendNative.Redeem(redeemRequestArchive)",
      "KagemushaRecursiveSpendRedeemInstructionArchive",
    ],
    "C# Kagemusha recursive redeem transaction builder",
  );
  assertContainsAll(
    source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/TransactionBuilderTests.cs"),
    [
      "AddInstructionAcceptsKagemushaInstructionArchiveFactories",
      "BuildSignedEmbedsKagemushaInstructionArchiveWithoutReframing",
      "KagemushaInstructionArchiveRejectsMalformedWrongTypeAndMismatchedType",
      "KagemushaInstructionArchiveAcceptsNativeAbi7RedeemInstructionFixture",
      "KagemushaInstructionType.RedeemRecursive",
      "KagemushaInstructionType.Transfer",
      "new KagemushaRecursiveSpendRedeemInstructionArchive(redeemArchive)",
      "Assert.Equal(archive, instruction.Payload)",
      'Assert.Equal("iroha_data_model::isi::offline::RedeemKagemushaRecursive", instruction.WireId)',
      'NoritoCodec.Encode("KagemushaRecursiveSpendRedeemRequestV1", new byte[] { 1, 2, 3 })',
      "compressed[22] = 1",
      "unsupportedFlags[39] = 0x08",
      "invalidFieldBitset[39] = 0x20",
      "WithHeaderPadding",
      "new byte[65]",
    ],
    "C# Kagemusha instruction transaction builder tests",
  );
});

test("Kagemusha JVM instruction archive transaction helpers stay wired", () => {
  assertContainsAll(
    source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaInstructionArchives.kt"),
    [
      "enum class KagemushaInstructionType",
      "TRANSFER(",
      "REDEEM_RECURSIVE(",
      '"KagemushaTransfer"',
      '"RedeemKagemushaRecursive"',
      '"iroha_data_model::isi::offline::KagemushaTransfer"',
      '"iroha_data_model::isi::offline::RedeemKagemushaRecursive"',
      "fun instructionBox(",
      "recursiveRedeemInstructionBox",
      "recursiveRedeemInstructionBoxFromRequest",
      "fun transactionPayload(",
      "recursiveRedeemTransactionPayload",
      "recursiveRedeemTransactionPayloadFromRequest",
      "KagemushaRecursiveSpendProver.redeemSpend(redeemRequestArchive)",
      "KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES",
      "NoritoHeader.decode(archive, SchemaHash.hash16(instructionType.wireName))",
      "decoded.header.compression == NoritoHeader.COMPRESSION_NONE",
      "decoded.header.payloadLength > 0",
      "decoded.header.validateChecksum(decoded.payload)",
      "InstructionBox.fromWirePayload(instructionType.wireName, archive)",
      "Executable.instructions(listOf(instructionBox(instructionType, instructionArchive)))",
    ],
    "Kotlin Kagemusha instruction archive transaction helper",
  );
  assertContainsAll(
    source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaInstructionArchives.java"),
    [
      "public enum InstructionType",
      "TRANSFER(",
      "REDEEM_RECURSIVE(",
      '"KagemushaTransfer"',
      '"RedeemKagemushaRecursive"',
      '"iroha_data_model::isi::offline::KagemushaTransfer"',
      '"iroha_data_model::isi::offline::RedeemKagemushaRecursive"',
      "public static InstructionBox instructionBox(",
      "recursiveRedeemInstructionBox",
      "recursiveRedeemInstructionBoxFromRequest",
      "public static TransactionPayload transactionPayload(",
      "recursiveRedeemTransactionPayload",
      "recursiveRedeemTransactionPayloadFromRequest",
      "KagemushaRecursiveSpendProver.redeemSpend(redeemRequestArchive)",
      "KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES",
      "NoritoHeader.decode(archive, SchemaHash.hash16(instructionType.wireName()))",
      "decoded.header().compression() != NoritoHeader.COMPRESSION_NONE",
      "decoded.header().payloadLength() == 0",
      "decoded.header().validateChecksum(decoded.payload())",
      "InstructionBox.fromWirePayload(instructionType.wireName(), archive)",
      "Executable.instructions(List.of(instructionBox(instructionType, instructionArchive)))",
    ],
    "Android Java Kagemusha instruction archive transaction helper",
  );
  assertContainsAll(
    source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaInstructionArchivesTest.kt"),
    [
      "instructionBox preserves redeem archive bytes and wire name",
      "transactionPayload wraps a single transfer archive instruction",
      "instructionBox accepts native ABI 7 redeem instruction fixture",
      "instructionBox rejects malformed wrong schema empty and tampered archives",
      "KagemushaInstructionType.REDEEM_RECURSIVE",
      "KagemushaInstructionType.TRANSFER",
      "assertContentEquals(archive, wire.payloadBytes)",
      "recursiveRedeemInstructionBoxFromRequest(byteArrayOf())",
      "recursiveRedeemTransactionPayloadFromRequest(",
      '"KagemushaRecursiveSpendRedeemRequestV1"',
      "tampered[tampered.lastIndex]",
      "compressed[22] = 1",
      "NoritoHeader.VARINT_OFFSETS",
      "NoritoHeader.FIELD_BITSET",
      "withNonZeroHeaderPadding",
    ],
    "Kotlin Kagemusha instruction archive transaction helper tests",
  );
  assertContainsAll(
    source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/tx/TransactionBuilderTests.java"),
    [
      "kagemushaInstructionArchivesBuildPayloads",
      "kagemushaInstructionArchivesAcceptAbi7Fixtures",
      "kagemushaInstructionArchivesRejectAdversarialInputs",
      "KagemushaInstructionArchives.InstructionType.REDEEM_RECURSIVE",
      "KagemushaInstructionArchives.InstructionType.TRANSFER",
      "KagemushaInstructionArchives.recursiveRedeemInstructionBox(archive)",
      "KagemushaInstructionArchives.transactionPayload(",
      "KagemushaInstructionArchives.recursiveRedeemInstructionBoxFromRequest(new byte[0])",
      "KagemushaInstructionArchives.recursiveRedeemTransactionPayloadFromRequest(",
      "Arrays.equals(transferArchive, transferWire.payloadBytes())",
      '"KagemushaRecursiveSpendRedeemRequestV1"',
      "tampered[tampered.length - 1] ^= 0x01",
      "compressed[22] = 1",
      "NoritoHeader.VARINT_OFFSETS",
      "NoritoHeader.FIELD_BITSET",
      "withNonZeroHeaderPadding",
    ],
    "Android Java Kagemusha instruction archive transaction helper tests",
  );
});

test("Kagemusha JavaScript instruction transaction builder stays wired", () => {
  for (const relative of [
    "javascript/iroha_js/src/transaction.js",
    "javascript/iroha_js/dist/transaction.js",
  ]) {
    const transactionSource = source(relative);
    assertContainsAll(
      transactionSource,
      [
        "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES",
        "iroha_data_model::isi::offline::KagemushaTransfer",
        "iroha_data_model::isi::offline::RedeemKagemushaRecursive",
        "validateKagemushaInstructionArchive",
        "noritoSchemaHash",
        "noritoCrc64",
        "bytesBase64 must be canonical standard base64",
        "buildKagemushaInstructionArchiveInstruction",
        "buildKagemushaInstructionTransaction",
        "buildKagemushaRecursiveRedeemTransaction",
        "KagemushaInstructionArchive",
        "KagemushaTransfer",
        "RedeemKagemushaRecursive",
      "instruction_type",
      'typeof type !== "string"',
      "normalizeExactMetadataString",
      "verifyingKey.id.backend",
      "verifyingKey.record.circuit_id",
      "must not contain surrounding whitespace",
      "kagemushaRecursiveSpendRedeem",
      "kagemushaRecursiveRedeem.redeemRequestArchive",
    ],
      `${relative} Kagemusha instruction transaction builder`,
    );
    assert.match(
      transactionSource,
      /buildKagemushaRecursiveRedeemTransaction[\s\S]*?kagemushaRecursiveSpendRedeem[\s\S]*?buildKagemushaInstructionTransaction/u,
      `${relative} must derive the redeem instruction before signing`,
    );
  }
  for (const relative of [
    "javascript/iroha_js/src/index.js",
    "javascript/iroha_js/dist/index.js",
  ]) {
    assertContainsAll(
      source(relative),
      [
        "buildKagemushaInstructionArchiveInstruction",
        "buildKagemushaInstructionTransaction",
        "buildKagemushaRecursiveRedeemTransaction",
      ],
      `${relative} Kagemusha instruction transaction exports`,
    );
  }
  assertContainsAll(
    source("javascript/iroha_js/index.d.ts"),
    [
      "KagemushaInstructionArchiveType",
      '"KagemushaTransfer"',
      '"RedeemKagemushaRecursive"',
      "KagemushaInstructionArchiveInput",
      "KagemushaInstructionTransactionInput",
      "KagemushaRecursiveRedeemTransactionBaseInput",
      "KagemushaRecursiveRedeemArchiveInput",
      "KagemushaRecursiveRedeemTransactionInput",
      "buildKagemushaInstructionArchiveInstruction",
      "buildKagemushaInstructionTransaction",
      "buildKagemushaRecursiveRedeemTransaction",
      "redeemRequestArchive: BinaryLike;",
      "redeem_request_archive: BinaryLike;",
      "requestArchive: BinaryLike;",
      "bytes_base64: string;",
    ],
    "JavaScript Kagemusha instruction transaction TypeScript declarations",
  );
  assertContainsAll(
    source("crates/iroha_js_host/src/lib.rs"),
    [
      "fn kagemusha_instruction_archive_from_json",
      'remove_case_insensitive(&mut map, "KagemushaInstructionArchive")',
      "KagemushaTransfer",
      "RedeemKagemushaRecursive",
      'ensure_kagemusha_recursive_archive_len(archive.len(), "Kagemusha instruction archive")',
      "KagemushaInstructionArchive.bytes_base64 must be canonical standard base64",
      "build_transaction_from_instructions_json_accepts_kagemusha_instruction_archive",
      "kagemusha_instruction_archive_json_rejects_adversarial_inputs",
    ],
    "iroha_js_host Kagemusha instruction archive decoder",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/transactionBuilder.test.js"),
    [
      "buildKagemushaInstructionArchiveInstruction normalizes archive bytes",
      "whitespaceInstructionType",
      "schema must match RedeemKagemushaRecursive",
      "checksum is invalid",
      "must not be compressed",
      "flags: 0x08",
      "flags: 0x20",
      "invalidPadding[40] = 0xff",
      "paddingLength: 65",
      "valid KagemushaTransfer Norito archive",
      "bytesBase64 must be canonical standard base64",
      "buildKagemushaInstructionTransaction wraps one archive instruction",
      "buildKagemushaRecursiveRedeemTransaction derives instruction before signing",
      "proof builders reject padded inline verifier-key metadata",
      "buildPrivateKaigiFeeSpend",
      "privateKaigiFeeSpend\\.verifyingKey\\.id\\.backend must not contain surrounding whitespace",
      "privateKaigiFeeSpend\\.verifyingKey\\.record\\.circuit_id must not contain surrounding whitespace",
      "instruction_type: \"KagemushaTransfer\"",
      "redeemRequestArchive must be a Buffer or ArrayBuffer view",
      "redeem native rejected",
    ],
    "JavaScript Kagemusha instruction transaction builder tests",
  );
});

test("Kagemusha Python instruction transaction builder stays wired", () => {
  assertContainsAll(
    source("python/iroha_python/src/iroha_python/kagemusha.py"),
    [
      "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_TRANSFER",
      "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE",
      "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES",
      "KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME",
      "KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME",
      "KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME",
      "KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES",
      "KagemushaInstructionArchiveType",
      "def _normalize_kagemusha_instruction_archive_type(",
      "if instruction_type not in KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES:",
      "def _assert_kagemusha_instruction_archive_schema(",
      "_norito_schema_hash(wire_name)",
      "def kagemusha_instruction_archive_instruction(",
      "def kagemusha_recursive_redeem_instruction(",
      "def build_kagemusha_instruction_transaction(",
      "def build_kagemusha_recursive_redeem_transaction(",
      '_norito_archive_bytes_named(instruction_archive, "instruction_archive")',
      'getattr(Instruction, "kagemusha_instruction_archive", None)',
      'getattr(Instruction, "kagemusha_recursive_redeem", None)',
      "build_signed_transaction",
      "instructions=(instruction,)",
    ],
    "Python Kagemusha instruction transaction builder",
  );
  assertContainsAll(
    source("python/iroha_python/src/iroha_python/__init__.py"),
    [
      "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_TRANSFER",
      "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE",
      "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES",
      "KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME",
      "KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME",
      "KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME",
      "KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES",
      "KagemushaInstructionArchiveType",
      "kagemusha_instruction_archive_instruction",
      "kagemusha_recursive_redeem_instruction",
      "build_kagemusha_instruction_transaction",
      "build_kagemusha_recursive_redeem_transaction",
    ],
    "Python Kagemusha instruction transaction root exports",
  );
  assertContainsAll(
    source("python/iroha_python/src/iroha_python/tx.py"),
    [
      "def kagemusha_instruction_archive(",
      "kagemusha_instruction_archive_instruction",
      "def kagemusha_recursive_redeem(",
      "kagemusha_recursive_redeem_instruction",
      "self.add_instruction(",
    ],
    "Python TransactionDraft Kagemusha helpers",
  );
  assertContainsAll(
    source("python/iroha_python/iroha_python_rs/src/lib.rs"),
    [
      "fn kagemusha_instruction_archive_box(",
      "fn kagemusha_instruction_archive(",
      "fn kagemusha_recursive_redeem(",
      "KagemushaTransfer",
      "RedeemKagemushaRecursive",
      "decode_from_bytes(instruction_archive)",
      "kagemusha_recursive_spend_redeem_instruction_from_request(request)",
      "kagemusha_instruction_archive_box_accepts_transfer_and_redeem_archives",
      "kagemusha_instruction_archive_box_rejects_adversarial_archives",
    ],
    "Python PyO3 Kagemusha instruction archive decoder",
  );
  assertContainsAll(
    source("python/iroha_python/tests/kagemusha_test.py"),
    [
      "test_kagemusha_instruction_archive_transaction_helpers_wrap_redeem_archive",
      "test_kagemusha_recursive_redeem_transaction_helper_derives_instruction_before_signing",
      "test_kagemusha_instruction_archive_transaction_helpers_reject_adversarial_inputs",
      '_shared_recursive_spend_abi7_archive("redeem_instruction")',
      '_shared_recursive_spend_abi7_archive("redeem_request")',
      "_instruction_archive_bytes(instruction)",
      "committed_instruction = kagemusha.kagemusha_instruction_archive_instruction",
      "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE",
      "instruction_archive must be a valid Norito archive",
      "schema must match RedeemKagemushaRecursive",
      "KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES",
      "whitespace_instruction_type",
      "compressed[22] = 1",
      "unsupported_flags[39] = 0x08",
      "invalid_field_bitset[39] = 0x20",
      "non_zero_padding.insert(40, 0x7F)",
      'excessive_padding[40:40] = b"\\x00" * 65',
      "bad_request_flags[39] = 0x20",
      "redeem_request_archive must be a valid Norito archive",
      "draft.kagemusha_recursive_redeem(request_archive)",
    ],
    "Python Kagemusha instruction transaction tests",
  );
});

test("recursive Kagemusha ABI-7 compact verifier surface stays in parity", () => {
  const rustBridge = source("crates/connect_norito_bridge/src/lib.rs");
  const header = source("crates/connect_norito_bridge/include/connect_norito_bridge.h");
  assertContainsAll(rustBridge, REQUIRED_RECURSIVE_COMPACT_C_SYMBOLS, "Rust recursive compact C bridge");
  assertContainsAll(header, REQUIRED_RECURSIVE_COMPACT_C_SYMBOLS, "C header recursive compact bridge");
  assertContainsAll(
    header,
    [
      "uint8_t* out_valid",
      "Input 2: Norito-archive bytes of `KagemushaRecursiveCompactVerifierKeysV1`.",
      "Shape-valid tokens with invalid proof bodies return success with `*out_valid = 0`.",
    ],
    "C header recursive compact verifier contract",
  );
  assertContainsAll(
    rustBridge,
    [
      "*out_valid = 0",
      "KagemushaRecursiveCompactUnavailable",
      "Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveCompactPaymentTokenProver_nativeVerifyRecursiveCompactPaymentToken",
      "Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveCompactPaymentTokenProver_nativeVerifyRecursiveCompactPaymentToken",
      "Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveCompactPaymentTokenProver_nativeRecursiveSpendCompactPaymentTokenFromBundle",
      "Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveCompactPaymentTokenProver_nativeRecursiveSpendCompactPaymentTokenFromBundle",
      "Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveCompactPaymentTokenProver_nativeVerifyRecursiveSpendCompactPaymentTokenProjection",
      "Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveCompactPaymentTokenProver_nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight",
      "Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveCompactPaymentTokenProver_nativeVerifyRecursiveSpendCompactPaymentTokenProjection",
      "Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveCompactPaymentTokenProver_nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight",
    ],
    "Rust recursive compact verifier implementation",
  );

  assertContainsAll(
    source("crates/iroha_js_host/src/lib.rs"),
    [
      ...REQUIRED_RECURSIVE_COMPACT_JS_METHODS.map((name) => `js_name = "${name}"`),
      'js_name = "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight"',
      "napi::Result<bool>",
      "Ok(false)",
      "is_kagemusha_recursive_compact_unavailable_error",
      "kagemusha_recursive_spend_compact_projection_verifier_js_host_rejects_malformed_inputs",
      "sentinel-spoofed recursive compact token must reject",
    ],
    "Node recursive compact verifier export",
  );

  for (const relative of ["javascript/iroha_js/src/crypto.js", "javascript/iroha_js/dist/crypto.js"]) {
    assertContainsAll(
      source(relative),
      [
        "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 7",
        "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
        "isKagemushaRecursiveCompactPaymentTokenNativeAvailable",
        "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable",
        "isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable",
        "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable",
        "hasKagemushaRecursiveCompactPaymentTokenVerifierNative",
        "hasKagemushaRecursiveSpendCompactPaymentTokenProjectionNative",
        "hasKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNative",
        ...REQUIRED_RECURSIVE_COMPACT_JS_METHODS,
        'typeof native.kagemushaVerifyRecursiveCompactPaymentToken !== "function"',
        'typeof native.kagemushaRecursiveSpendCompactPaymentTokenFromBundle !== "function"',
        'typeof native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection !== "function"',
        'typeof native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight !== "function"',
        "native.kagemushaVerifyRecursiveCompactPaymentToken(",
        "const recursiveCompactVerifierKeys = toOwnedKagemushaArchiveBuffer(",
        '"recursiveCompactVerifierKeysArchive"',
        "native.kagemushaRecursiveSpendCompactPaymentTokenFromBundle(",
        "native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(",
        "native.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
        "Object.is(blockHeight, -0)",
        "/\\b(?:archive|Norito|probe)\\b/i.test(error.message)",
        "toOwnedKagemushaArchiveBuffer",
        'const compactToken = toOwnedKagemushaArchiveBuffer(',
        '"compactTokenArchive"',
        'const recursiveCompactKeyArtifacts = toOwnedKagemushaArchiveBuffer(',
        '"recursiveCompactKeyArtifactsArchive"',
        'const bundle = toOwnedKagemushaArchiveBuffer(bundleArchive, "bundleArchive")',
        'const verifierRecord = toOwnedKagemushaArchiveBuffer(',
        '"verifierRecordArchive"',
        "recursive compact Kagemusha payment-token verifier requires native bridge ABI 7 with the compact verifier symbol",
        "recursive spend compact Kagemusha payment-token projection requires native bridge ABI 7 with the compact projection symbol",
        "recursive spend compact Kagemusha payment-token projection verifier requires native bridge ABI 7 with the compact projection verifier symbols",
        "kagemushaVerifyRecursiveCompactPaymentToken returned a non-boolean result",
        "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection returned a non-boolean result",
      ],
      `${relative} recursive compact verifier gate`,
    );
  }
  for (const relative of ["javascript/iroha_js/src/crypto.browser.js", "javascript/iroha_js/dist/crypto.browser.js"]) {
    assertContainsAll(
      source(relative),
      [
        "isKagemushaRecursiveCompactPaymentTokenNativeAvailable",
        "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable",
        "isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable",
        "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable",
        ...REQUIRED_RECURSIVE_COMPACT_JS_METHODS,
        'unsupported("kagemushaVerifyRecursiveCompactPaymentToken")',
        'unsupported("kagemushaRecursiveSpendCompactPaymentTokenFromBundle")',
        'unsupported("kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection")',
      ],
      `${relative} recursive compact browser stubs`,
    );
  }
  for (const relative of ["javascript/iroha_js/src/index.js", "javascript/iroha_js/dist/index.js"]) {
    assertContainsAll(source(relative), REQUIRED_RECURSIVE_COMPACT_JS_METHODS, `${relative} recursive compact exports`);
  }
  assertContainsAll(
    source("javascript/iroha_js/index.d.ts"),
    [
      "isKagemushaRecursiveCompactPaymentTokenNativeAvailable(): boolean",
      "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(): boolean",
      "isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable(): boolean",
      "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable(): boolean",
      "kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(",
      "recursiveCompactKeyArtifactsArchive: BinaryLike",
      "kagemushaVerifyRecursiveCompactPaymentToken(",
      "recursiveCompactVerifierKeysArchive: BinaryLike",
      "kagemushaRecursiveSpendCompactPaymentTokenFromBundle(",
      "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(",
      "blockHeight?: number | bigint | null",
    ],
    "JavaScript TypeScript recursive compact declarations",
  );
  const dts = source("javascript/iroha_js/index.d.ts");
  assert.match(
    dts,
    /export function kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\(\s*recordBundleArchive: BinaryLike,\s*pallasOpenEnvelopesArchive: BinaryLike,\s*recursiveCompactKeyArtifactsArchive: BinaryLike,\s*\): Buffer;/u,
    "JavaScript TypeScript recursive compact prover declaration must require key artifacts",
  );
  assert.match(
    dts,
    /export function kagemushaVerifyRecursiveCompactPaymentToken\(\s*compactTokenArchive: BinaryLike,\s*recursiveCompactVerifierKeysArchive: BinaryLike,\s*\): boolean;/u,
    "JavaScript TypeScript recursive compact verifier declaration must require verifier keys",
  );
  assert.doesNotMatch(
    dts,
    /recursiveCompact(?:KeyArtifactsArchive|VerifierKeysArchive)\?:\s*BinaryLike/u,
    "JavaScript TypeScript recursive compact key packages must not be optional",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/kagemushaRecursiveSpend.test.js"),
    [
      "kagemushaNoritoFrameWithPayload",
      "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable",
      "isKagemushaRecursiveCompactPaymentTokenVerifierNativeAvailable(), true",
      "with the compact verifier symbol",
      "compactTokenArchive must be a valid Norito archive",
      "compactTokenArchive must contain a non-empty Norito payload",
      "recursiveCompactKeyArtifactsArchive must be a valid Norito archive",
      "recursiveCompactVerifierKeysArchive must be a valid Norito archive",
      "kagemushaVerifyRecursiveCompactPaymentToken returned a non-boolean result",
      "Kagemusha recursive spend compact projection probes availability and validates native output",
      "Kagemusha recursive spend compact projection verifier probes and delegates",
      "bundleArchive must contain a non-empty Norito payload",
      "verifierRecordArchive must be a valid Norito archive",
    ],
    "JavaScript recursive compact verifier tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package declarations expose recursive compact key-package signatures",
      'packageJson.exports["./crypto"].types, "./index.d.ts"',
      "package declarations keep accumulator digests native-owned",
      "recursive accumulator digests must remain native-owned",
    ],
    "JavaScript package recursive compact declaration tests",
  );

  assertContainsAll(
    source("python/iroha_python/src/iroha_python/kagemusha.py"),
    [
      "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 7",
      "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
      "is_kagemusha_recursive_compact_payment_token_prover_available",
      "is_kagemusha_recursive_compact_payment_token_verifier_available",
      "is_kagemusha_pallas_open_envelope_builder_available",
      "is_kagemusha_recursive_spend_compact_payment_token_projection_available",
      "is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available",
      "_RECURSIVE_COMPACT_TOKEN_METHOD",
      '"kagemusha_prove_verified_recursive_compact_payment_token"',
      '"_with_records_and_pallas_open_envelopes"',
      "_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD",
      '"kagemusha_verify_recursive_compact_payment_token"',
      "_PALLAS_OPEN_ENVELOPE_BUILDER_METHOD",
      '"kagemusha_build_pallas_open_envelopes_archive"',
      "_PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD",
      '"kagemusha_build_previous_proof_open_envelopes_archive"',
      "_RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD",
      '"kagemusha_recursive_spend_compact_payment_token_from_bundle"',
      "_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD",
      '"kagemusha_verify_recursive_spend_compact_payment_token_projection"',
      "_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_AT_HEIGHT_METHOD",
      '"kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"',
      "globals()[_RECURSIVE_COMPACT_TOKEN_METHOD]",
      "globals()[_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD]",
      "globals()[_PALLAS_OPEN_ENVELOPE_BUILDER_METHOD]",
      "globals()[_PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD]",
      "globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD]",
      "globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD]",
      '("archive", "norito", "probe")',
      "Kagemusha Pallas open-envelope builders require native bridge ABI 7",
      '_norito_archive_bytes_named(record_bundle_archive, "record_bundle_archive")',
      '"previous_bundle_archive"',
      '_assert_kagemusha_norito_archive(compact_token, "compact_token_archive")',
      '_archive_bytes_named(',
      "recursive_compact_verifier_keys_archive,",
      '_assert_kagemusha_norito_archive(',
      '_assert_kagemusha_norito_archive(verifier_record, "verifier_record_archive")',
      '_norito_archive_bytes_named(bundle_archive, "bundle_archive")',
      "block_height must be non-negative",
      "returned non-boolean result",
    ],
    "Python recursive compact verifier and Pallas builder surface",
  );
  assertContainsAll(
    source("python/iroha_python/src/iroha_python/__init__.py"),
    [
      "is_kagemusha_recursive_compact_payment_token_prover_available",
      "is_kagemusha_recursive_compact_payment_token_verifier_available",
      "is_kagemusha_pallas_open_envelope_builder_available",
      "kagemusha_build_pallas_open_envelopes_archive",
      "kagemusha_build_previous_proof_open_envelopes_archive",
      "kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes",
      "kagemusha_verify_recursive_compact_payment_token",
    ],
    "Python package recursive compact root re-exports",
  );
  assertContainsAll(
    source("python/iroha_python/tests/kagemusha_test.py"),
    [
      "_kagemusha_norito_frame_with_payload",
      "compact_token_archive must be a valid Norito archive",
      "compact_token_archive must contain a non-empty Norito payload",
      "recursive_compact_key_artifacts_archive must be a valid Norito archive",
      "recursive_compact_verifier_keys_archive must be a valid Norito archive",
      "previous_bundle_archive must be a valid Norito archive",
      "previous_bundle_archive must contain a non-empty Norito payload",
      "is_kagemusha_pallas_open_envelope_builder_available",
      "kagemusha_build_pallas_open_envelopes_archive",
      "kagemusha_build_previous_proof_open_envelopes_archive",
      "Kagemusha recursive compact proof unavailable",
      "Kagemusha recursive compact verifier unavailable",
      "recursive spend compact Kagemusha payment-token projection requires native bridge ABI 7",
      "compact projection verifier symbols",
      "test_recursive_spend_compact_projection_verifier_probes_and_delegates",
      "verifier_record_archive must be a valid Norito archive",
      "returned non-boolean result",
    ],
    "Python recursive compact verifier tests",
  );
  assertContainsAll(
    source("python/iroha_python/iroha_python_rs/src/lib.rs"),
    [
      ...REQUIRED_RECURSIVE_COMPACT_PYTHON_METHODS.map((name) => `name = "${name}"`),
      'name = "kagemusha_build_pallas_open_envelopes_archive"',
      'name = "kagemusha_build_previous_proof_open_envelopes_archive"',
      'name = "kagemusha_verify_recursive_spend_compact_payment_token_projection"',
      'name = "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"',
      "is_kagemusha_recursive_compact_unavailable_error",
      "kagemusha_pallas_open_envelopes_from_record_bundle",
      "kagemusha_recursive_previous_proof_open_envelope_metadata",
      "kagemusha_recursive_spend_compact_projection_verifier_python_rejects_malformed_inputs",
      "sentinel-spoofed recursive compact token must reject",
    ],
    "Python PyO3 recursive compact exports",
  );

  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift"),
    [
      "requiredNativeBridgeAbiVersion: UInt32 = 7",
      'recursiveCompactCircuitIdV1 = "kagemusha-recursive-compact-v1"',
      "public static var isVerifierNativeAvailable",
      "public static var isProjectionNativeAvailable",
      "public static var isProjectionVerifierNativeAvailable",
      "isKagemushaRecursiveCompactPaymentTokenVerifierAvailable",
      "isKagemushaRecursiveSpendCompactPaymentTokenProjectionAvailable",
      "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable",
      "public static func verifyRecursiveCompactPaymentToken",
      "public static func recursiveSpendCompactPaymentTokenFromBundle",
      "public static func verifyRecursiveSpendCompactPaymentTokenProjection",
      "bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveCompactPaymentTokenVerifierAvailable",
      "bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendCompactPaymentTokenProjectionAvailable",
      ".isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable",
      "try requireValidInputArchive(",
      "try requireValidRecursiveCompactTokenArchive(token)",
      "Kagemusha recursive spend bundle archive must be a valid Norito archive.",
      "Kagemusha recursive spend bundle archive must contain a non-empty Norito payload.",
      "Kagemusha verifier record archive must be a valid Norito archive.",
      "Kagemusha verifier record archive must contain a non-empty Norito payload.",
      "requireValidRecursiveCompactTokenArchive(compactTokenArchive)",
      "recursiveCompactKeyArtifactsArchive: Data",
      "recursiveCompactVerifierKeysArchive: Data",
      "Kagemusha verified fold record bundle archive must be a valid Norito archive.",
      "Kagemusha Pallas open-envelope archive must contain a non-empty Norito payload.",
      "Kagemusha recursive compact key-artifacts archive must be a valid Norito archive.",
      "Kagemusha recursive compact verifier-keys archive must be a valid Norito archive.",
      "Kagemusha recursive compact-token archive must be a valid Norito archive.",
      "Kagemusha recursive compact-token archive must contain a non-empty Norito payload.",
      "Kagemusha recursive compact-token archive was rejected by the native verifier.",
    ],
    "Swift recursive compact wrapper",
  );
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"),
    [
      "connect_norito_kagemusha_verify_recursive_compact_payment_token",
      "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle",
      "probeKagemushaRecursiveCompactPaymentTokenVerifierFunction",
      "probeKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierFunction",
      "kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn != nil",
      "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn",
      "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn",
      "kagemushaVerifyRecursiveCompactPaymentTokenFn != nil",
      "isKagemushaRecursiveCompactPaymentTokenVerifierAvailable",
      "isKagemushaRecursiveSpendCompactPaymentTokenProjectionAvailable",
      "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable",
      "kagemushaRecursiveCompactPaymentTokenVerifierNativeProbeOk",
      "kagemushaRecursiveSpendCompactProjectionNativeProbeOk",
      "kagemushaRecursiveSpendCompactProjectionVerifierNativeProbeOk",
      "normalizeKagemushaRecursiveCompactVerifierOutput",
      "invalidKagemushaVerifierOutput",
    ],
    "Swift recursive compact bridge probe",
  );
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift"),
    [
      "testVerifyRejectsMalformedCompactTokenArchiveBeforeBridgeCall",
      "testVerifyRejectsEmptyPayloadCompactTokenArchiveBeforeBridgeCall",
      "testVerifyRejectsInvalidVerifierKeysArchiveBeforeBridgeCall",
      "testRejectsMalformedInputArchivesBeforeBridgeCall",
      "testRejectsEmptyPayloadInputArchivesBeforeBridgeCall",
      "testRejectsInvalidKeyArtifactsArchiveBeforeBridgeCall",
      "testRejectsMalformedNativeOutput",
      "testRejectsEmptyPayloadNativeOutput",
      "malformedKagemushaNoritoArchives",
      "compressed[22] = 0x01",
      "unsupportedFlags[39] = NoritoHeader.varintOffsets",
      "invalidFieldBitset[39] = NoritoHeader.fieldBitset",
      "kagemushaNoritoFrameWithHeaderPadding",
      "Data([0x7f])",
      "Data(repeating: 0, count: 65)",
      "testReturnsValidNativeOutput",
      "testProjectionRejectsMalformedBundleArchiveBeforeBridgeCall",
      "testProjectionRequiresBridgeAfterInputValidation",
      "testProjectionReturnsValidNativeOutput",
      "testProjectionVerifierRejectsMalformedVerifierRecordBeforeBridgeCall",
      "testProjectionVerifierRequiresNativeAvailabilityAfterInputValidation",
      "testProjectionVerifierReturnsNativeBoolean",
      "validKagemushaNoritoArchive",
      "testVerifyReturnsNativeBoolean",
      "testVerifyRequiresVerifierNativeAvailabilityAfterInputValidation",
      "testNativeBridgeRejectsInvalidVerifierBooleanOutput",
      "valid: 2",
      "invalidKagemushaVerifierOutput",
      "testVerifyNativeRejectionIsVerificationRejected",
    ],
    "Swift recursive compact verifier tests",
  );

  assertContainsAll(
    source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt"),
    [
      "REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 7",
      "fun isVerifierNativeAvailable(): Boolean",
      "fun isProjectionVerifierNativeAvailable(): Boolean",
      "fun verifyRecursiveCompactPaymentToken(",
      "recursiveCompactVerifierKeysArchive: ByteArray?",
      "recursiveCompactKeyArtifactsArchive: ByteArray?",
      "fun verifyRecursiveSpendCompactPaymentTokenProjection(",
      "fun verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
      "fun recursiveSpendCompactPaymentTokenFromBundle(",
      "private val nativeVerifierAvailable: Boolean = loadVerifierLibrary()",
      "private val nativeProjectionVerifierAvailable: Boolean = loadProjectionVerifierLibrary()",
      "check(nativeVerifierAvailable)",
      "check(nativeProjectionVerifierAvailable)",
      "nativeRecursiveSpendCompactPaymentTokenFromBundle(ByteArray(0))",
      "private fun loadVerifierLibrary(): Boolean",
      "private fun loadProjectionVerifierLibrary(): Boolean",
      'val compactToken = ownedNativeInput(compactTokenArchive, "compactTokenArchive")',
      'val verifierKeys =',
      'val verifierRecord = ownedNativeInput(verifierRecordArchive, "verifierRecordArchive")',
      'val bundle = ownedNativeInput(bundleArchive, "bundleArchive")',
      "KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)",
      "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)",
      "nativeVerifyRecursiveCompactPaymentToken(ByteArray(0), ByteArray(0))",
      "nativeVerifyRecursiveSpendCompactPaymentTokenProjection(ByteArray(0), ByteArray(0))",
      "nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
    ],
    "Kotlin recursive compact wrapper",
  );
  assertContainsAll(
    source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java"),
    [
      "REQUIRED_BRIDGE_ABI_VERSION = 7",
      "REQUIRED_NATIVE_BRIDGE_ABI_VERSION = REQUIRED_BRIDGE_ABI_VERSION",
      "public static boolean isVerifierNativeAvailable()",
      "public static boolean isProjectionVerifierNativeAvailable()",
      "public static boolean verifyRecursiveCompactPaymentToken(",
      "final byte[] recursiveCompactVerifierKeysArchive",
      "final byte[] recursiveCompactKeyArtifactsArchive",
      "public static boolean verifyRecursiveSpendCompactPaymentTokenProjection(",
      "public static boolean verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
      "public static byte[] recursiveSpendCompactPaymentTokenFromBundle",
      "NATIVE_VERIFIER_AVAILABLE = loadVerifierLibrary()",
      "NATIVE_PROJECTION_VERIFIER_AVAILABLE = loadProjectionVerifierLibrary()",
      "requireVerifierNative()",
      "requireProjectionVerifierNative()",
      "nativeRecursiveSpendCompactPaymentTokenFromBundle(new byte[0])",
      "private static boolean loadVerifierLibrary()",
      "private static boolean loadProjectionVerifierLibrary()",
      'final byte[] compactToken = ownedNativeInput(compactTokenArchive, "compactTokenArchive")',
      'final byte[] verifierKeys =',
      'final byte[] bundle = ownedNativeInput(bundleArchive, "bundleArchive")',
      "KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)",
      "KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)",
      "nativeVerifyRecursiveCompactPaymentToken(new byte[0], new byte[0])",
      "nativeVerifyRecursiveSpendCompactPaymentTokenProjection(",
      "nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(",
    ],
    "Android Java recursive compact wrapper",
  );
  assertContainsAll(
    source("csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"),
    [
      "RecursiveCompactRequiredNativeBridgeAbiVersion = 7",
      "IsPallasOpenEnvelopeBuilderAvailable",
      "public static KagemushaPallasOpenEnvelopesArchive BuildPallasOpenEnvelopesArchive",
      "public static KagemushaPreviousProofOpenEnvelopesArchive BuildPreviousProofOpenEnvelopesArchive",
      "IsRecursiveCompactPaymentTokenVerifierAvailable",
      "IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable",
      "public static bool VerifyRecursiveCompactPaymentToken(",
      "ReadOnlySpan<byte> recursiveCompactVerifierKeysArchive",
      "ReadOnlySpan<byte> recursiveCompactKeyArtifactsArchive",
      "public static bool VerifyRecursiveSpendCompactPaymentTokenProjection(",
      "public static KagemushaRecursiveCompactPaymentTokenArchive RecursiveSpendCompactPaymentTokenFromBundle",
      "TryProbeRecursiveSpendCompactPaymentTokenProjectionVerifierSymbol",
      "TryProbePallasOpenEnvelopeBuilderSymbols",
      "NativeBuildPallasOpenEnvelopesArchive",
      "NativeBuildPreviousProofOpenEnvelopesArchive",
      "NativeVerifyRecursiveSpendCompactPaymentTokenProjection",
      "NativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight",
      "RequireValidInputArchive",
      "RequireValidRecursiveCompactTokenArchive(compactToken)",
      "PrivacyNative.IsNoritoV1Archive(compactTokenArchive)",
      "Record bundle archive",
      "Pallas open-envelopes archive",
      "Recursive compact key artifacts archive",
      "Recursive compact verifier keys archive",
      "Recursive spend bundle archive",
      "must be a valid Norito archive.",
      "must contain a non-empty Norito payload.",
      "RequireValidNativeOutput(symbol, result)",
      "returned invalid Norito archive",
      "returned empty Norito payload",
      "Compact token archive must be a valid Norito archive.",
      "Compact token archive must contain a non-empty Norito payload.",
      "connect_norito_kagemusha_verify_recursive_compact_payment_token",
      "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle",
      "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection",
      "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height",
    ],
    "C# recursive compact wrapper",
  );
  assert.doesNotMatch(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift"),
    /recursiveCompactKeyArtifactsArchive:\s*Data\s*=\s*Data\s*\(/u,
    "Swift recursive compact prover must not default missing key artifacts to empty Data",
  );
  assert.doesNotMatch(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift"),
    /recursiveCompactVerifierKeysArchive:\s*Data\s*=\s*Data\s*\(/u,
    "Swift recursive compact verifier must not default missing verifier keys to empty Data",
  );
  assert.match(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift"),
    /public\s+static\s+func\s+proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*\(\s*recordBundleArchive:\s*Data\s*,\s*pallasOpenEnvelopesArchive:\s*Data\s*,\s*recursiveCompactKeyArtifactsArchive:\s*Data\s*\)\s*throws\s*->\s*Data/su,
    "Swift recursive compact prover must require key artifacts",
  );
  assert.match(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift"),
    /public\s+static\s+func\s+verifyRecursiveCompactPaymentToken\s*\(\s*compactTokenArchive:\s*Data\s*,\s*recursiveCompactVerifierKeysArchive:\s*Data\s*\)\s*throws\s*->\s*Bool/su,
    "Swift recursive compact verifier must require verifier keys",
  );
  assert.doesNotMatch(
    source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt"),
    /fun\s+proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*\(\s*recordBundleArchive:\s*ByteArray\?,\s*pallasOpenEnvelopesArchive:\s*ByteArray\?,?\s*\)\s*:\s*ByteArray/su,
    "Kotlin recursive compact prover must not expose a stale two-archive overload",
  );
  assert.doesNotMatch(
    source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt"),
    /fun\s+verifyRecursiveCompactPaymentToken\s*\(\s*compactTokenArchive:\s*ByteArray\?,?\s*\)\s*:\s*Boolean/su,
    "Kotlin recursive compact verifier must not expose a stale one-archive overload",
  );
  assert.doesNotMatch(
    source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java"),
    /public\s+static\s+byte\[\]\s+proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*\(\s*final\s+byte\[\]\s+recordBundleArchive\s*,\s*final\s+byte\[\]\s+pallasOpenEnvelopesArchive\s*\)/su,
    "Android Java recursive compact prover must not expose a stale two-archive overload",
  );
  assert.doesNotMatch(
    source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java"),
    /public\s+static\s+boolean\s+verifyRecursiveCompactPaymentToken\s*\(\s*final\s+byte\[\]\s+compactTokenArchive\s*\)/su,
    "Android Java recursive compact verifier must not expose a stale one-archive overload",
  );
  assert.doesNotMatch(
    source("csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"),
    /public\s+static\s+KagemushaRecursiveCompactPaymentTokenArchive\s+ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes\s*\(\s*ReadOnlySpan<byte>\s+recordBundleArchive\s*,\s*ReadOnlySpan<byte>\s+pallasOpenEnvelopesArchive\s*\)/su,
    "C# recursive compact prover must not expose a stale two-archive overload",
  );
  assert.doesNotMatch(
    source("csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"),
    /public\s+static\s+bool\s+VerifyRecursiveCompactPaymentToken\s*\(\s*ReadOnlySpan<byte>\s+compactTokenArchive\s*\)/su,
    "C# recursive compact verifier must not expose a stale one-archive overload",
  );
  assertContainsAll(
    source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs"),
    [
      "validRecursiveCompactVerifierKeys",
      "IsPallasOpenEnvelopeBuilderAvailable",
      "Recursive compact verifier keys archive must be a valid Norito archive",
      "VerifyRecursiveSpendCompactPaymentTokenProjection",
      "PallasOpenEnvelopeBuildersRejectMalformedInputsBeforeLoadingNativeBridge",
      "PallasOpenEnvelopeBuildersRejectOversizedInputsBeforeLoadingNativeBridge",
      "PallasOpenEnvelopeBuildersRejectEmptyPayloadInputsBeforeLoadingNativeBridge",
      "PallasOpenEnvelopeBuilderReadBridgeOutputRejectsMalformedNoritoSuccessOutput",
      "PallasOpenEnvelopeBuilderReadBridgeOutputRejectsEmptyPayloadNoritoSuccessOutput",
      "BuildPallasOpenEnvelopesArchive",
      "BuildPreviousProofOpenEnvelopesArchive",
      "Previous recursive proof bundle archive must be a valid Norito archive",
      "connect_norito_kagemusha_build_pallas_open_envelopes_archive returned invalid Norito archive",
      "connect_norito_kagemusha_build_previous_proof_open_envelopes_archive returned empty Norito payload",
      "RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge",
      "RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
      "RecursiveSpendCompactProjectionRejectsInvalidBundleBeforeLoadingNativeBridge",
      "RecursiveSpendCompactProjectionVerifierRejectsInvalidInputsBeforeLoadingNativeBridge",
      "RecursiveSpendNativeReadBridgeOutputRejectsMalformedNoritoSuccessOutput",
      "RecursiveSpendNativeReadBridgeOutputRejectsEmptyPayloadNoritoSuccessOutput",
      "RecursiveSpendNativeReadBridgeOutputReturnsValidNoritoSuccessOutput",
      "AssertRejectsMalformedBridgeOutput",
      "AssertRejectsMalformedBridgeOutput(compressed)",
      "AssertRejectsMalformedBridgeOutput(unsupportedFlags)",
      "AssertRejectsMalformedBridgeOutput(invalidFieldBitset)",
      "valid Norito archive",
      "non-empty Norito payload",
      "KagemushaNoritoFrameWithPayload",
      "KagemushaNoritoFrame",
    ],
    "C# recursive compact verifier tests",
  );
});

test("Kagemusha JavaScript record-backed native builders stay in parity", () => {
  assertContainsAll(
    source("crates/iroha_js_host/src/lib.rs"),
    [
      ...REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS.map((name) => `js_name = "${name}"`),
      ...REQUIRED_KAGEMUSHA_PALLAS_OPEN_ENVELOPE_BUILDER_JS_METHODS.map((name) => `js_name = "${name}"`),
      "prove_verified_kagemusha_compact_payment_token_from_record_bundle",
      "prove_verified_kagemusha_recursive_aggregation_proof_bundle_from_record_bundle_and_pallas_open_envelope_archive",
      "kagemusha_pallas_open_envelopes_from_record_bundle",
      "kagemusha_recursive_previous_proof_open_envelope_metadata",
      "kagemusha-recursive-spend-hop-open-v1-{hop_index}",
      "kagemusha-recursive-spend-previous-open-v1",
      "KAGEMUSHA_FOLDED_CIRCUIT_ID",
      "KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID",
    ],
    "Node record-backed Kagemusha prover and Pallas builder exports",
  );

  for (const relative of ["javascript/iroha_js/src/crypto.js", "javascript/iroha_js/dist/crypto.js"]) {
    assertContainsAll(
      source(relative),
      [
        "isKagemushaCompactPaymentTokenNativeAvailable",
        "isKagemushaRecursiveAggregationProofBundleNativeAvailable",
        "isKagemushaPallasOpenEnvelopeBuilderNativeAvailable",
        ...REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS,
        ...REQUIRED_KAGEMUSHA_PALLAS_OPEN_ENVELOPE_BUILDER_JS_METHODS,
        'typeof native.kagemushaProveVerifiedCompactPaymentTokenWithRecords !== "function"',
        'typeof native.kagemushaBuildPallasOpenEnvelopesArchive !== "function"',
        'typeof native.kagemushaBuildPreviousProofOpenEnvelopesArchive !== "function"',
        "native.kagemushaProveVerifiedCompactPaymentTokenWithRecords(",
        "native.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(",
        "native.kagemushaBuildPallasOpenEnvelopesArchive(",
        "native.kagemushaBuildPreviousProofOpenEnvelopesArchive(",
        "toOwnedKagemushaArchiveBuffer",
        'const recordBundle = toOwnedKagemushaArchiveBuffer(',
        'const pallasOpenEnvelopes = toOwnedKagemushaArchiveBuffer(',
        'const previousBundle = toOwnedKagemushaArchiveBuffer(',
        '"recordBundleArchive"',
        '"pallasOpenEnvelopesArchive"',
        '"previousBundleArchive"',
        "Kagemusha compact payment-token prover requires native bridge ABI 6",
        "Kagemusha recursive aggregation proof-bundle prover requires native bridge ABI 6",
        "Kagemusha Pallas open-envelope builders require native bridge ABI 7",
      ],
      `${relative} record-backed Kagemusha and Pallas builder wrappers`,
    );
  }
  for (const relative of ["javascript/iroha_js/src/crypto.browser.js", "javascript/iroha_js/dist/crypto.browser.js"]) {
    assertContainsAll(
      source(relative),
      [
        "isKagemushaCompactPaymentTokenNativeAvailable",
        "isKagemushaRecursiveAggregationProofBundleNativeAvailable",
        "isKagemushaPallasOpenEnvelopeBuilderNativeAvailable",
        ...REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS,
        ...REQUIRED_KAGEMUSHA_PALLAS_OPEN_ENVELOPE_BUILDER_JS_METHODS,
        'unsupported("kagemushaProveVerifiedCompactPaymentTokenWithRecords")',
        'unsupported("kagemushaBuildPallasOpenEnvelopesArchive")',
        'unsupported("kagemushaBuildPreviousProofOpenEnvelopesArchive")',
      ],
      `${relative} record-backed Kagemusha and Pallas builder browser stubs`,
    );
  }
  for (const relative of ["javascript/iroha_js/src/index.js", "javascript/iroha_js/dist/index.js"]) {
    assertContainsAll(
      source(relative),
      [
        "isKagemushaPallasOpenEnvelopeBuilderNativeAvailable",
        ...REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS,
        ...REQUIRED_KAGEMUSHA_PALLAS_OPEN_ENVELOPE_BUILDER_JS_METHODS,
      ],
      `${relative} record-backed Kagemusha and Pallas builder exports`,
    );
  }
  assertContainsAll(
    source("javascript/iroha_js/index.d.ts"),
    [
      "isKagemushaCompactPaymentTokenNativeAvailable(): boolean",
      "isKagemushaRecursiveAggregationProofBundleNativeAvailable(): boolean",
      "isKagemushaPallasOpenEnvelopeBuilderNativeAvailable(): boolean",
      "kagemushaProveVerifiedCompactPaymentTokenWithRecords(",
      "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(",
      "kagemushaBuildPallasOpenEnvelopesArchive(",
      "kagemushaBuildPreviousProofOpenEnvelopesArchive(",
    ],
    "JavaScript record-backed Kagemusha and Pallas builder TypeScript declarations",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/kagemushaRecursiveSpend.test.js"),
    [
      "Kagemusha record-backed JS builders probe availability and validate native output",
      "Kagemusha Pallas open-envelope JS builders probe availability and validate native output",
      "recordBundleArchive must be a valid Norito archive",
      "previousBundleArchive must be a valid Norito archive",
      "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
      "returned invalid Norito archive",
      "returned empty Norito payload",
      "Kagemusha Pallas open-envelope builders require native bridge ABI 7",
    ],
    "JavaScript record-backed Kagemusha and Pallas builder runtime tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/crypto.browser.test.js"),
    [
      "browser build must not expose native compact-token prover",
      "browser build must not expose native recursive aggregation prover",
      "browser build must not expose native Pallas open-envelope builders",
      ...REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS,
      ...REQUIRED_KAGEMUSHA_PALLAS_OPEN_ENVELOPE_BUILDER_JS_METHODS,
    ],
    "JavaScript record-backed Kagemusha and Pallas builder browser tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "isKagemushaCompactPaymentTokenNativeAvailable",
      "isKagemushaRecursiveAggregationProofBundleNativeAvailable",
      "isKagemushaPallasOpenEnvelopeBuilderNativeAvailable",
      ...REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS,
      ...REQUIRED_KAGEMUSHA_PALLAS_OPEN_ENVELOPE_BUILDER_JS_METHODS,
    ],
    "JavaScript package record-backed Kagemusha and Pallas builder exports",
  );
});

test("recursive Kagemusha ABI-6 availability probes require transition-profile, boundary, and lineage-witness helpers", () => {
  for (const relative of ["javascript/iroha_js/src/crypto.js", "javascript/iroha_js/dist/crypto.js"]) {
    assertContainsAll(
      source(relative),
      [
        '"kagemushaRecursiveSpendTransitionProfileInit"',
        '"kagemushaRecursiveSpendTransitionProfileAppend"',
        '"kagemushaRecursiveSpendLineageAppendBoundary"',
        '"kagemushaRecursiveSpendLineageWitnessFromInitResult"',
        '"kagemushaRecursiveSpendLineageWitnessAppendResult"',
      ],
      `${relative} availability probe`,
    );
  }
  assertContainsAll(
    source("python/iroha_python/src/iroha_python/kagemusha.py"),
    [
      '"kagemusha_recursive_spend_transition_profile_init"',
      '"kagemusha_recursive_spend_transition_profile_append"',
      '"kagemusha_recursive_spend_lineage_append_boundary"',
      '"kagemusha_recursive_spend_lineage_witness_from_init_result"',
      '"kagemusha_recursive_spend_lineage_witness_append_result"',
    ],
    "Python availability probe",
  );
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"),
    [
      "kagemushaRecursiveSpendNativeProbeOk",
      "probeKagemushaArchiveFunction(kagemushaRecursiveSpendTransitionProfileInitFn)",
      "probeKagemushaArchiveFunction(kagemushaRecursiveSpendTransitionProfileAppendFn)",
      "probeKagemushaArchiveFunction(kagemushaRecursiveSpendLineageAppendBoundaryFn)",
      "probeKagemushaLineageWitnessFromInitResultFunction(\n                kagemushaRecursiveSpendLineageWitnessFromInitResultFn",
      "probeKagemushaLineageWitnessAppendResultFunction(\n                kagemushaRecursiveSpendLineageWitnessAppendResultFn",
    ],
    "Swift availability probe",
  );
  assertContainsAll(
    source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"),
    [
      "expectIllegalArgumentProbe(() -> nativeTransitionProfileInit(new byte[0]))",
      "expectIllegalArgumentProbe(() -> nativeTransitionProfileAppend(new byte[0]))",
      "expectIllegalArgumentProbe(() -> nativeLineageAppendBoundary(new byte[0]))",
      "() -> nativeLineageWitnessFromInitResult(probe, probe)",
      "() -> nativeLineageWitnessAppendResult(probe, probe, probe)",
    ],
    "Android Java availability probe",
  );
  assertContainsAll(
    source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"),
    [
      "expectIllegalArgumentProbe { nativeTransitionProfileInit(ByteArray(0)) }",
      "expectIllegalArgumentProbe { nativeTransitionProfileAppend(ByteArray(0)) }",
      "expectIllegalArgumentProbe { nativeLineageAppendBoundary(ByteArray(0)) }",
      "nativeLineageWitnessFromInitResult(probe, probe)",
      "nativeLineageWitnessAppendResult(probe, probe, probe)",
    ],
    "Kotlin availability probe",
  );
  assertContainsAll(
    source("csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"),
    [
      "Probe(NativeTransitionProfileInit)",
      "Probe(NativeTransitionProfileAppend)",
      "Probe(NativeLineageAppendBoundary)",
      "Probe((NativeArchivePairCall)NativeLineageWitnessFromInitResult)",
      "Probe((NativeArchiveTripleCall)NativeLineageWitnessAppendResult)",
    ],
    "C# availability probe",
  );
});

test("recursive Kagemusha ABI probes reject unsafe and out-of-range versions", () => {
  for (const relative of [
    "javascript/iroha_js/src/crypto.js",
    "javascript/iroha_js/dist/crypto.js",
  ]) {
    assertContainsAll(
      source(relative),
      [
        "const KAGEMUSHA_MAX_NATIVE_BRIDGE_ABI_VERSION = 0xffff_ffff",
        "Number.isSafeInteger(version)",
        "version >= 0",
        "version <= KAGEMUSHA_MAX_NATIVE_BRIDGE_ABI_VERSION",
      ],
      `${relative} Kagemusha ABI probe bounds`,
    );
  }

  for (const relative of [
    "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
    "javascript/iroha_js/test/package_dist.test.js",
  ]) {
    assertContainsAll(
      source(relative),
      [
        "Number.NaN",
        "Number.POSITIVE_INFINITY",
        "Number.MAX_SAFE_INTEGER + 1",
        "0x1_0000_0000",
        "6.5",
        "-1",
        "isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false",
      ],
      `${relative} Kagemusha ABI probe tests`,
    );
  }

  assertContainsAll(
    source("python/iroha_python/src/iroha_python/kagemusha.py"),
    [
      "KAGEMUSHA_MAX_NATIVE_BRIDGE_ABI_VERSION = 0xFFFF_FFFF",
      "isinstance(version, bool)",
      "not isinstance(version, int)",
      "version < 0",
      "version > KAGEMUSHA_MAX_NATIVE_BRIDGE_ABI_VERSION",
    ],
    "Python Kagemusha ABI probe bounds",
  );
  assertContainsAll(
    source("python/iroha_python/tests/kagemusha_test.py"),
    [
      "test_recursive_kagemusha_availability_requires_bridge_abi_6",
      '"6"',
      "6.5",
      "0x1_0000_0000",
      "10**100",
      "is_kagemusha_recursive_compact_payment_token_prover_available",
      "is_kagemusha_recursive_compact_payment_token_verifier_available",
    ],
    "Python Kagemusha ABI probe tests",
  );
});

test("recursive Kagemusha append-opening preflight domain is exposed across SDKs", () => {
  const expectedRust = [
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
    APPEND_OPENINGS_PREFLIGHT_DOMAIN,
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
    APPEND_BOUNDARY_DOMAIN,
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
    APPEND_BOUNDARY_CHAIN_ASSET_DOMAIN,
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
    APPEND_BOUNDARY_FINAL_NOTE_DOMAIN,
  ];
  const expectedJs = [
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
    APPEND_OPENINGS_PREFLIGHT_DOMAIN,
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
    APPEND_BOUNDARY_DOMAIN,
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
    APPEND_BOUNDARY_CHAIN_ASSET_DOMAIN,
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
    APPEND_BOUNDARY_FINAL_NOTE_DOMAIN,
  ];
  const expectedPython = [
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
    APPEND_OPENINGS_PREFLIGHT_DOMAIN,
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
    APPEND_BOUNDARY_DOMAIN,
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
    APPEND_BOUNDARY_CHAIN_ASSET_DOMAIN,
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
    APPEND_BOUNDARY_FINAL_NOTE_DOMAIN,
  ];
  const expectedSwift = [
    "recursiveSpendLineageAppendOpeningsPreflightDomainV1",
    APPEND_OPENINGS_PREFLIGHT_DOMAIN,
    "recursiveSpendLineageAppendBoundaryDomainV1",
    APPEND_BOUNDARY_DOMAIN,
    "recursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1",
    APPEND_BOUNDARY_CHAIN_ASSET_DOMAIN,
    "recursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1",
    APPEND_BOUNDARY_FINAL_NOTE_DOMAIN,
  ];
  const expectedJvm = [
    "RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
    APPEND_OPENINGS_PREFLIGHT_DOMAIN,
    "RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
    APPEND_BOUNDARY_DOMAIN,
    "RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
    APPEND_BOUNDARY_CHAIN_ASSET_DOMAIN,
    "RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
    APPEND_BOUNDARY_FINAL_NOTE_DOMAIN,
  ];
  const expectedCsharp = [
    "RecursiveSpendLineageAppendOpeningsPreflightDomainV1",
    APPEND_OPENINGS_PREFLIGHT_DOMAIN,
    "RecursiveSpendLineageAppendBoundaryDomainV1",
    APPEND_BOUNDARY_DOMAIN,
    "RecursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1",
    APPEND_BOUNDARY_CHAIN_ASSET_DOMAIN,
    "RecursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1",
    APPEND_BOUNDARY_FINAL_NOTE_DOMAIN,
  ];

  assertContainsAll(
    source("crates/iroha_data_model/src/offline/mod.rs"),
    expectedRust,
    "Rust data-model append-opening preflight domain",
  );
  for (const relative of [
    "javascript/iroha_js/src/crypto.js",
    "javascript/iroha_js/dist/crypto.js",
    "javascript/iroha_js/src/crypto.browser.js",
    "javascript/iroha_js/dist/crypto.browser.js",
    "javascript/iroha_js/index.d.ts",
  ]) {
    assertContainsAll(source(relative), expectedJs, `${relative} append-opening preflight domain`);
  }
  for (const relative of ["javascript/iroha_js/src/index.js", "javascript/iroha_js/dist/index.js"]) {
    assertContainsAll(
      source(relative),
      [
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
      ],
      `${relative} append-opening preflight re-export`,
    );
  }
  assertContainsAll(
    source("python/iroha_python/src/iroha_python/kagemusha.py"),
    expectedPython,
    "Python append-opening preflight domain",
  );
  assertContainsAll(
    source("python/iroha_python/src/iroha_python/__init__.py"),
    [
      "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
      "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
      "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
      "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
    ],
    "Python package append-opening preflight export",
  );
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift"),
    expectedSwift,
    "Swift append-opening preflight domain",
  );
  assertContainsAll(
    source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"),
    expectedJvm,
    "Android Java append-opening preflight domain",
  );
  assertContainsAll(
    source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"),
    expectedJvm,
    "Kotlin JVM append-opening preflight domain",
  );
  assertContainsAll(
    source("csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"),
    expectedCsharp,
    "C# append-opening preflight domain",
  );
});

test("Kagemusha JavaScript SDK runner rejects non-Node-20 overrides before tests", () => {
  assertRunnerRejectsNodeMajor(
    "ci/check_kagemusha_recursive_spend_js_sdk.sh",
    "KAGEMUSHA_RECURSIVE_SPEND_JS_SDK_NODE_BIN",
    "Kagemusha JavaScript SDK runner",
  );
});

test("Kagemusha Python SDK runner rejects non-3.11 overrides before native builds", () => {
  assertRunnerRejectsPythonMajor(
    "ci/check_kagemusha_recursive_spend_python_sdk.sh",
    "KAGEMUSHA_RECURSIVE_SPEND_PYTHON_BIN",
    "Kagemusha Python SDK runner",
  );
});

test("Kagemusha Swift SDK runner propagates parse failures", () => {
  assertRunnerPropagatesSwiftParseFailure(
    "ci/check_kagemusha_recursive_spend_swift_sdk.sh",
    "KAGEMUSHA_RECURSIVE_SPEND_SWIFTC_BIN",
    "Kagemusha Swift SDK runner",
  );
});

test("Kagemusha JVM SDK runner rejects non-JDK-21 overrides before tests", () => {
  assertRunnerRejectsJavaHome(
    "ci/check_kagemusha_recursive_spend_jvm_sdk.sh",
    "KAGEMUSHA_RECURSIVE_SPEND_JVM_JAVA_HOME",
    "Kagemusha JVM SDK runner",
  );
});

test("Kagemusha C# SDK runner rejects non-.NET-8 overrides before tests", () => {
  assertRunnerRejectsDotnetSdk(
    "ci/check_kagemusha_recursive_spend_csharp_sdk.sh",
    "KAGEMUSHA_RECURSIVE_SPEND_DOTNET_BIN",
    "Kagemusha C# SDK runner",
  );
});

test("Kagemusha C# SDK runner prints host and bridge evidence before tests", () => {
  assertRunnerPrintsDotnetAndBridgeEvidence(
    "ci/check_kagemusha_recursive_spend_csharp_sdk.sh",
    "KAGEMUSHA_RECURSIVE_SPEND_DOTNET_BIN",
    "Kagemusha C# SDK runner",
  );
});

test("recursive Kagemusha witnessless Reserved-lineage policy stays enabled in public docs", () => {
  const docs = [
    "docs/source/offline_kagemusha.md",
    "roadmap.md",
    "IrohaSwift/README.md",
    "java/iroha_android/README.md",
    "kotlin/README.md",
    "csharp/README.md",
    "javascript/iroha_js/README.md",
    "python/iroha_python/README.md",
  ];
  const forbiddenClaims = [
    /witnessless\s+Reserved-lineage\s+redeem\s+requests\s+are\s+emitted\s+only\s+inside\s+the\s+one-hop/iu,
    /metadata-valid\s+one-hop\s+Reserved-lineage\s+requests\s+can\s+serialize\s+witnessless\s+redeem/iu,
    /chain-admission\s+checks\.\s+Those\s+checks\s+admit\s+only\s+the\s+one-hop\s+verifier-slice/iu,
    /WITNESSLESS_MAX_HOPS_V1[^.\n]*0/iu,
    /TRANSITION_CIRCUIT_WIRED_V1[^.\n]*false/iu,
    /witnessless\s+Reserved-lineage[^.\n]*(not\s+admitted|disabled)/iu,
  ];

  const rustDataModel = source("crates/iroha_data_model/src/offline/mod.rs");
  assert.match(
    rustDataModel,
    /KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1:\s*u32\s*=\s*64\s*;/u,
    "Rust data model must expose the 64-hop witnessless Reserved-lineage cap",
  );
  assert.match(
    rustDataModel,
    /KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1:\s*bool\s*=\s*true\s*;/u,
    "Rust data model must expose the wired Reserved-lineage transition circuit",
  );

  for (const relativePath of docs) {
    const text = source(relativePath);
    assert.match(
      text,
      /(?:WITNESSLESS_MAX_HOPS_V1[^.\n]*64|64-hop|64\s+hops|witnessless[^.\n]*Reserved-lineage[^.\n]*(enabled|available|admitted))/iu,
      `${relativePath} must document the enabled witnessless Reserved-lineage boundary`,
    );
    for (const forbidden of forbiddenClaims) {
      assert.doesNotMatch(text, forbidden, `${relativePath} contains stale disabled witnessless claim`);
    }
  }
});

test("recursive Kagemusha SDK docs expose compact projection verifier APIs", () => {
  const commonReadmeSnippets = [
    "recursive-spend compact projection verifier",
    "raw Norito compact-token and verifier-record archives",
    "native boolean receiver result",
  ];
  const perSdkSnippets = new Map([
    [
      "IrohaSwift/README.md",
      [
        "verifyRecursiveSpendCompactPaymentTokenProjection(compactTokenArchive:verifierRecordArchive:blockHeight:)",
        "isProjectionVerifierNativeAvailable",
      ],
    ],
    [
      "kotlin/README.md",
      [
        "verifyRecursiveSpendCompactPaymentTokenProjection(...)",
        "verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(...)",
        "isProjectionVerifierNativeAvailable()",
      ],
    ],
    [
      "java/iroha_android/README.md",
      [
        "verifyRecursiveSpendCompactPaymentTokenProjection(...)",
        "verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(...)",
        "isProjectionVerifierNativeAvailable()",
      ],
    ],
    [
      "csharp/README.md",
      [
        "VerifyRecursiveSpendCompactPaymentTokenProjection(...)",
        "IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable()",
      ],
    ],
    [
      "javascript/iroha_js/README.md",
      [
        "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(...)",
        "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable()",
      ],
    ],
    [
      "python/iroha_python/README.md",
      [
        "kagemusha_verify_recursive_spend_compact_payment_token_projection(...)",
        "is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available()",
      ],
    ],
  ]);

  for (const [relativePath, snippets] of perSdkSnippets.entries()) {
    assertContainsAll(
      source(relativePath).replace(/\s+/gu, " "),
      [...commonReadmeSnippets, ...snippets],
      `${relativePath} compact projection verifier docs`,
    );
  }

  assertContainsAll(
    source("docs/source/offline_kagemusha.md").replace(/\s+/gu, " "),
    [
      "Swift, Kotlin/JVM, Java Android, JavaScript/Node, Python, and C#",
      "typed recursive-spend compact projection verifier facades",
      "ABI-7 compact projection verifier symbols",
    ],
    "offline Kagemusha compact projection verifier docs",
  );
});

test("recursive Kagemusha SDK docs expose instruction transaction builders", () => {
  const commonReadmeSnippets = [
    "KagemushaTransfer",
    "RedeemKagemushaRecursive",
    "valid Norito archives",
    "empty, malformed, tampered, or wrong-type instruction archives",
    "recursive redeem derivation inside",
  ];
  const perSdkSnippets = new Map([
    [
      "IrohaSwift/README.md",
      [
        "KagemushaInstructionTransactionRequest",
        "IrohaSDK.buildKagemushaRecursiveRedeem(...)",
      ],
    ],
    [
      "kotlin/README.md",
      [
        "KagemushaInstructionArchives",
        "builds a single archived instruction transaction payload",
        "derives the redeem instruction from a native recursive redeem request",
      ],
    ],
    [
      "java/iroha_android/README.md",
      [
        "KagemushaInstructionArchives",
        "builds a single archived instruction transaction payload",
        "derives the redeem instruction from a native recursive redeem request",
      ],
    ],
    [
      "csharp/README.md",
      [
        "TransactionInstruction.KagemushaInstructionArchive(...)",
        "KagemushaInstructionArchiveInstruction",
        "TransactionBuilder.KagemushaInstructionArchive(...)",
        "TransactionBuilder.KagemushaRecursiveRedeem(...)",
      ],
    ],
    [
      "javascript/iroha_js/README.md",
      [
        "buildKagemushaInstructionArchiveInstruction({ instructionType, instructionArchive })",
        "buildKagemushaInstructionTransaction(...)",
        "buildKagemushaRecursiveRedeemTransaction(...)",
      ],
    ],
    [
      "python/iroha_python/README.md",
      [
        "kagemusha_instruction_archive_instruction(instruction_type, instruction_archive)",
        "build_kagemusha_instruction_transaction(...)",
        "build_kagemusha_recursive_redeem_transaction(...)",
        "TransactionDraft.kagemusha_instruction_archive(...)",
        "TransactionDraft.kagemusha_recursive_redeem(...)",
      ],
    ],
  ]);

  for (const [relativePath, snippets] of perSdkSnippets.entries()) {
    assertContainsAll(
      source(relativePath).replace(/\s+/gu, " "),
      [...commonReadmeSnippets, ...snippets],
      `${relativePath} instruction transaction builder docs`,
    );
  }

  assertContainsAll(
    source("docs/source/offline_kagemusha.md").replace(/\s+/gu, " "),
    [
      "typed archived-instruction transaction surface",
      "KagemushaTransfer",
      "RedeemKagemushaRecursive",
      "valid Norito archives",
      "preserve their canonical bytes rather than re-framing them",
      "empty, malformed, tampered, or wrong-type instruction archives",
      "Recursive redeem derivation inside the transaction helper",
      "native recursive redeem request",
      "signs exactly one `RedeemKagemushaRecursive` instruction",
    ],
    "offline Kagemusha instruction transaction docs",
  );
});

test("recursive Kagemusha Python SDK docs name compact root helpers exactly", () => {
  const readme = source("python/iroha_python/README.md").replace(/\s+/gu, " ");
  assertContainsAll(
    readme,
    [
      "Import the helpers from `iroha_python` or `iroha_python.kagemusha`",
      "kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(record_bundle_archive, pallas_open_envelopes_archive, recursive_compact_key_artifacts_archive)",
      "kagemusha_verify_recursive_compact_payment_token(compact_token_archive, recursive_compact_verifier_keys_archive)",
      "is_kagemusha_recursive_compact_payment_token_prover_available()",
      "is_kagemusha_recursive_compact_payment_token_verifier_available()",
    ],
    "Python recursive compact README root helper docs",
  );
});

test("Kagemusha production readiness negative controls pin ABI-7 compact launch boundaries", () => {
  const readiness = source("ci/check_kagemusha_production_readiness.sh");
  const readinessScript = source("scripts/kagemusha_production_readiness.py");
  const readinessTests = source("scripts/tests/kagemusha_production_readiness_test.py");
  const dataModel = source("crates/iroha_data_model/src/offline/mod.rs");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const verifierWitnessProfile = "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-64x4";
  const expectedModes = [
    "--negative-control-abi6-manifest",
    "--negative-control-abi6-manifest-direct-invalid-json",
    "--negative-control-abi6-manifest-direct-duplicate-json-key",
    "--negative-control-abi6-manifest-direct-nonfinite-json",
    "--negative-control-abi6-manifest-direct-object-shape",
    "--negative-control-abi6-manifest-direct-closed-schema",
    "--negative-control-abi6-manifest-direct-nested-closed-schema",
    "--negative-control-abi6-manifest-direct-nested-value-binding",
    "--negative-control-abi6-manifest-direct-operation-value-binding",
    "--negative-control-abi6-manifest-integer-scalars",
    "--negative-control-abi6-manifest-limit-integer-scalars",
    "--negative-control-abi6-manifest-direct-operation-shape",
    "--negative-control-abi6-manifest-direct-limits-shape",
    "--negative-control-abi6-manifest-direct-modes-shape",
    "--negative-control-abi6-manifest-operation-shape",
    "--negative-control-abi6-manifest-closed-schema",
    "--negative-control-abi6-manifest-nested-closed-schema",
    "--negative-control-abi6-manifest-nested-shape",
    "--negative-control-abi6-manifest-nested-value-binding",
    "--negative-control-abi6-manifest-file-aliases",
    "--negative-control-abi6-manifest-ancestor-aliases",
    "--negative-control-abi7-source-marker-file-aliases",
    "--negative-control-compact-open",
    "--negative-control-abi7-core-contract-open",
    "--negative-control-abi7-one-hop-runtime-keygen-fallback",
    "--negative-control-abi7-append-runtime-keygen-fallback",
    "--negative-control-abi7-bridge-unavailable-mapping",
    "--negative-control-abi7-offline-doc-one-hop-boundary",
    "--negative-control-offline-doc-evidence-filename-exactness",
    "--negative-control-offline-doc-compact-generator-log-exactness",
    "--negative-control-offline-doc-release-bundle-output-exactness",
    "--negative-control-offline-doc-verifier-profile-exactness",
    "--negative-control-compact-key-release-tooling",
    "--negative-control-compact-key-evidence",
    "--negative-control-compact-key-evidence-path-aliases",
    "--negative-control-compact-key-artifact-prefix-binding",
    "--negative-control-compact-key-artifact-size-binding",
    "--negative-control-compact-key-evidence-json-size-limit",
    "--negative-control-compact-key-readiness-artifact-open-path-binding",
    "--negative-control-compact-key-placeholder-artifacts",
    "--negative-control-compact-key-generator-log-digest-binding",
    "--negative-control-compact-key-generator-log-size-limit",
    "--negative-control-compact-key-generator-log-open-path-binding",
    "--negative-control-compact-key-helper-validation-dir-create-failure",
    "--negative-control-compact-key-helper-validation-strict-json-write",
    "--negative-control-compact-key-helper-validation-temp-write-failure",
    "--negative-control-compact-key-helper-validation-temp-cleanup-after-write-failure",
    "--negative-control-compact-key-helper-validation-temp-cleanup-failure",
    "--negative-control-compact-key-helper-validation-temp-cleanup-identity",
    "--negative-control-compact-key-helper-direct-artifact-dir-secret-paths",
    "--negative-control-compact-key-helper-direct-artifact-dir-metadata-failure",
    "--negative-control-compact-key-helper-direct-hash-shape",
    "--negative-control-compact-key-helper-direct-hash-read-failure",
    "--negative-control-compact-key-helper-generator-log-strict-read",
    "--negative-control-compact-key-helper-artifact-open-path-binding",
    "--negative-control-compact-key-helper-future-skew",
    "--negative-control-compact-key-helper-output-early-preflight",
    "--negative-control-compact-key-helper-output-file-metadata-failure",
    "--negative-control-compact-key-helper-output-hardlink-metadata-failure",
    "--negative-control-compact-key-helper-output-parent-create-failure",
    "--negative-control-compact-key-helper-output-parent-sync-identity",
    "--negative-control-compact-key-helper-output-post-write-preflight",
    "--negative-control-compact-key-helper-output-published-cleanup-identity",
    "--negative-control-compact-key-helper-output-readback-failure",
    "--negative-control-compact-key-helper-output-readback-open-path-binding",
    "--negative-control-compact-key-helper-output-readback-verification",
    "--negative-control-compact-key-helper-output-temp-cleanup-failure",
    "--negative-control-compact-key-helper-output-temp-cleanup-identity",
    "--negative-control-compact-key-helper-output-write-failure",
    "--negative-control-compact-key-helper-strict-json-write",
    "--negative-control-compact-key-finalizer-exit-marker",
    "--negative-control-compact-key-finalizer-timestamp-raw",
    "--negative-control-compact-key-finalizer-future-skew",
    "--negative-control-compact-key-finalizer-publish-readback",
    "--negative-control-compact-key-finalizer-publish-rollback-identity",
    "--negative-control-compact-key-finalizer-publish-rollback-cleanup-report",
    "--negative-control-compact-key-finalizer-publish-dir-sync-identity",
    "--negative-control-compact-key-finalizer-temp-cleanup-identity",
    "--negative-control-compact-key-finalizer-temp-cleanup-report",
    "--negative-control-compact-key-staged-runner-exit-marker",
    "--negative-control-compact-key-staged-runner-readback",
    "--negative-control-compact-key-staged-runner-parent-sync-identity",
    "--negative-control-compact-key-staged-runner-log-install-parent-sync-identity",
    "--negative-control-compact-key-staged-runner-cleanup-identity",
    "--negative-control-compact-key-staged-runner-published-cleanup-report",
    "--negative-control-compact-key-staged-runner-child-log-file",
    "--negative-control-compact-key-staged-runner-supervisor-output-pipe",
    "--negative-control-compact-key-staged-runner-execution-log-sha256",
    "--negative-control-compact-key-staged-runner-resume-replace-conflict",
    "--negative-control-doc-route",
    "--negative-control-evidence-helper-path-aliases",
    "--negative-control-json-duplicate-keys",
    "--negative-control-kagemusha-readiness-json-read-failure",
    "--negative-control-kagemusha-readiness-json-open-path-binding",
    "--negative-control-kagemusha-readiness-release-json-direct-secret-paths",
    "--negative-control-kagemusha-readiness-release-json-direct-path-aliases",
    "--negative-control-kagemusha-readiness-release-json-hardlink-metadata-failure",
    "--negative-control-kagemusha-readiness-release-json-file-metadata-failure",
    "--negative-control-kagemusha-readiness-release-json-size-limit",
    "--negative-control-kagemusha-readiness-release-json-open-path-binding",
    "--negative-control-kagemusha-readiness-repo-root-aliases",
    "--negative-control-kagemusha-readiness-repo-root-direct-secret-paths",
    "--negative-control-kagemusha-readiness-repo-root-metadata-failure",
    "--negative-control-kagemusha-readiness-repo-root-resolve-failure",
    "--negative-control-kagemusha-readiness-rollup",
    "--negative-control-kagemusha-readiness-rollup-path-safety",
    "--negative-control-kagemusha-readiness-source-marker-direct-secret-paths",
    "--negative-control-kagemusha-readiness-source-marker-direct-path-aliases",
    "--negative-control-kagemusha-readiness-source-marker-hardlink-metadata-failure",
    "--negative-control-kagemusha-readiness-source-marker-file-metadata-failure",
    "--negative-control-kagemusha-readiness-source-marker-read-preflight",
    "--negative-control-kagemusha-readiness-source-marker-open-path-binding",
    "--negative-control-kagemusha-readiness-source-marker-non-utf8-read",
    "--negative-control-kagemusha-readiness-source-marker-size-limit",
    "--negative-control-kagemusha-readiness-trusted-signer-sanitization",
    "--negative-control-kagemusha-readiness-android-report-secret-redaction",
    "--negative-control-kagemusha-readiness-android-zero-binding-digest",
    "--negative-control-kagemusha-readiness-trust-root-section-preflight",
    "--negative-control-kagemusha-readiness-android-root-discovery-read-failure",
    "--negative-control-kagemusha-readiness-summary-output-aliases",
    "--negative-control-kagemusha-readiness-summary-output-dangling-alias",
    "--negative-control-kagemusha-readiness-summary-output-ancestor",
    "--negative-control-kagemusha-readiness-summary-output-parent-is-dir-preflight",
    "--negative-control-kagemusha-readiness-summary-output-parent-metadata-failure",
    "--negative-control-kagemusha-readiness-summary-output-parent-create-failure",
    "--negative-control-kagemusha-readiness-summary-output-post-create-parent-preflight",
    "--negative-control-kagemusha-readiness-summary-output-regular-file",
    "--negative-control-kagemusha-readiness-summary-output-file-metadata-failure",
    "--negative-control-kagemusha-readiness-summary-output-hardlink-metadata-failure",
    "--negative-control-kagemusha-readiness-summary-output-direct-secret-paths",
    "--negative-control-kagemusha-readiness-summary-output-write-failure",
    "--negative-control-kagemusha-readiness-summary-output-temp-cleanup-failure",
    "--negative-control-kagemusha-readiness-summary-output-temp-cleanup-identity",
    "--negative-control-kagemusha-readiness-summary-output-published-cleanup-identity",
    "--negative-control-kagemusha-readiness-summary-output-strict-json-write",
    "--negative-control-kagemusha-readiness-summary-output-size-limit",
    "--negative-control-kagemusha-readiness-summary-output-readback-verification",
    "--negative-control-kagemusha-readiness-summary-output-readback-failure",
    "--negative-control-kagemusha-readiness-summary-output-readback-size-limit",
    "--negative-control-kagemusha-readiness-summary-output-readback-open-path-binding",
    "--negative-control-kagemusha-readiness-summary-output-parent-sync-identity",
    "--negative-control-kagemusha-readiness-summary-output-post-write-preflight",
    "--negative-control-lineage-key-release-tooling",
    "--negative-control-lineage-proof-evidence",
    "--negative-control-lineage-proof-evidence-path-aliases",
    "--negative-control-lineage-proof-local-secret-paths",
    "--negative-control-lineage-proof-local-path-aliases",
    "--negative-control-lineage-proof-local-ancestor-aliases",
    "--negative-control-lineage-proof-local-hardlink-metadata-failure",
    "--negative-control-lineage-proof-local-file-metadata-failure",
    "--negative-control-lineage-proof-artifact-binding",
    "--negative-control-lineage-proof-artifact-is-file-preflight",
    "--negative-control-lineage-proof-file-aliases",
    "--negative-control-lineage-proof-future-skew",
    "--negative-control-lineage-proof-artifact-prefix-binding",
    "--negative-control-lineage-proof-command-canonical",
    "--negative-control-lineage-proof-scalar-types",
    "--negative-control-lineage-proof-artifact-size-binding",
    "--negative-control-lineage-proof-evidence-json-size-limit",
    "--negative-control-lineage-proof-readiness-artifact-open-path-binding",
    "--negative-control-lineage-proof-helper-timestamp-raw",
    "--negative-control-lineage-proof-helper-future-skew",
    "--negative-control-lineage-proof-helper-strict-json-write",
    "--negative-control-lineage-proof-helper-artifact-open-path-binding",
    "--negative-control-lineage-proof-helper-direct-secret-paths",
    "--negative-control-lineage-proof-helper-direct-hash-shape",
    "--negative-control-lineage-proof-helper-direct-hash-read-failure",
    "--negative-control-lineage-proof-helper-direct-artifact-dir-secret-paths",
    "--negative-control-lineage-proof-helper-direct-artifact-dir-metadata-failure",
    "--negative-control-lineage-proof-helper-direct-proof-log-secret-paths",
    "--negative-control-lineage-proof-helper-direct-output-preflight-secret-paths",
    "--negative-control-lineage-proof-helper-validation-dir-aliases",
    "--negative-control-lineage-proof-helper-validation-dir-create-failure",
    "--negative-control-lineage-proof-helper-validation-strict-json-write",
    "--negative-control-lineage-proof-helper-validation-temp-write-failure",
    "--negative-control-lineage-proof-helper-validation-temp-cleanup-after-write-failure",
    "--negative-control-lineage-proof-helper-validation-temp-cleanup-failure",
    "--negative-control-lineage-proof-helper-validation-temp-cleanup-identity",
    "--negative-control-lineage-proof-helper-input-corridor",
    "--negative-control-lineage-proof-helper-input-corridor-resolve-failure",
    "--negative-control-lineage-proof-helper-output-aliases",
    "--negative-control-lineage-proof-helper-output-dangling-alias",
    "--negative-control-lineage-proof-helper-output-ancestor",
    "--negative-control-lineage-proof-helper-output-parent-is-dir-preflight",
    "--negative-control-lineage-proof-helper-output-parent-metadata-failure",
    "--negative-control-lineage-proof-helper-output-parent-create-failure",
    "--negative-control-lineage-proof-helper-output-post-create-parent-preflight",
    "--negative-control-lineage-proof-helper-output-validate-parent-create-failure",
    "--negative-control-lineage-proof-helper-output-file-metadata-failure",
    "--negative-control-lineage-proof-helper-output-hardlink-metadata-failure",
    "--negative-control-lineage-proof-helper-output-early-preflight",
    "--negative-control-lineage-proof-helper-output-write-failure",
    "--negative-control-lineage-proof-helper-output-temp-cleanup-failure",
    "--negative-control-lineage-proof-helper-output-temp-cleanup-identity",
    "--negative-control-lineage-proof-helper-output-published-cleanup-identity",
    "--negative-control-lineage-proof-helper-output-readback-verification",
    "--negative-control-lineage-proof-helper-output-readback-failure",
    "--negative-control-lineage-proof-helper-output-readback-open-path-binding",
    "--negative-control-lineage-proof-helper-output-post-write-preflight",
    "--negative-control-lineage-proof-helper-output-corridor-resolve-failure",
    "--negative-control-lineage-proof-finalizer-exit-marker",
    "--negative-control-lineage-proof-finalizer-timestamp-raw",
    "--negative-control-lineage-proof-finalizer-future-skew",
    "--negative-control-lineage-proof-finalizer-publish-readback",
    "--negative-control-lineage-proof-finalizer-publish-rollback-identity",
    "--negative-control-lineage-proof-finalizer-publish-rollback-cleanup-report",
    "--negative-control-lineage-proof-finalizer-publish-dir-sync-identity",
    "--negative-control-lineage-proof-finalizer-temp-cleanup-identity",
    "--negative-control-lineage-proof-finalizer-temp-cleanup-report",
    "--negative-control-lineage-proof-staged-runner-exit-marker",
    "--negative-control-lineage-proof-staged-runner-readback",
    "--negative-control-lineage-proof-staged-runner-parent-sync-identity",
    "--negative-control-lineage-proof-staged-runner-log-install-parent-sync-identity",
    "--negative-control-lineage-proof-staged-runner-cleanup-identity",
    "--negative-control-lineage-proof-staged-runner-published-cleanup-report",
    "--negative-control-lineage-proof-staged-runner-child-log-file",
    "--negative-control-lineage-proof-staged-runner-supervisor-output-pipe",
    "--negative-control-lineage-proof-staged-runner-execution-log-sha256",
    "--negative-control-lineage-proof-staged-runner-resume-replace-conflict",
    "--negative-control-lineage-proof-log-exact",
    "--negative-control-lineage-proof-log-size-limit",
    "--negative-control-lineage-proof-log-is-file-preflight",
    "--negative-control-lineage-proof-log-text-preflight",
    "--negative-control-lineage-proof-log-open-path-binding",
    "--negative-control-lineage-proof-evidence-filename",
    "--negative-control-lineage-proof-evidence-output-parent-sync-identity",
    "--negative-control-lineage-proof-closed-schema",
    "--negative-control-lineage-proof-evidence-helper",
    "--negative-control-lineage-proof-timestamp-raw",
    "--negative-control-lineage-proof-readiness-direct-hash-shape",
    "--negative-control-lineage-proof-readiness-direct-hash-read-failure",
    "--negative-control-sdk-default",
    "--negative-control-pallas-envelope-type",
    "--negative-control-staged-path-aliases",
    "--negative-control-compact-key-command-canonical",
    "--negative-control-compact-key-scalar-types",
    "--negative-control-compact-key-timestamp-raw",
    "--negative-control-compact-key-evidence-filename",
    "--negative-control-compact-key-closed-schema",
    "--negative-control-android-signed-evidence-summary-identity-fields",
    "--negative-control-android-device-lab-artifact-binding",
    "--negative-control-android-device-lab-abi6-probe-status-exactness",
    "--negative-control-android-device-lab-ancestor-cwd-failure",
    "--negative-control-android-device-lab-ancestor-metadata-failure",
    "--negative-control-android-device-lab-ancestor-is-symlink-preflight",
    "--negative-control-android-device-lab-ancestor-exists-preflight",
    "--negative-control-android-device-lab-attestation-binding",
    "--negative-control-android-device-lab-attestation-chain-binding",
    "--negative-control-android-device-lab-attestation-chain-shape",
    "--negative-control-android-device-lab-attestation-slot-binding",
    "--negative-control-android-device-lab-attestation-schema",
    "--negative-control-android-device-lab-attestation-report",
    "--negative-control-android-device-lab-attestation-report-level-fields",
    "--negative-control-android-device-lab-attestation-report-result-level-binding",
    "--negative-control-android-device-lab-attestation-report-result-status-binding",
    "--negative-control-android-device-lab-attestation-status-exactness",
    "--negative-control-android-device-lab-attestation-result-slot-keymint-binding",
    "--negative-control-android-device-lab-capture-attestation-result-binding",
    "--negative-control-android-device-lab-capture-chain-binding",
    "--negative-control-android-device-lab-capture-summary-parent-sync-identity",
    "--negative-control-android-device-lab-capture-summary-published-cleanup-identity",
    "--negative-control-android-device-lab-capture-summary-temp-cleanup-identity",
    "--negative-control-android-device-lab-cli-secret-paths",
    "--negative-control-android-device-lab-d2d-transcript",
    "--negative-control-android-device-lab-d2d-path-root",
    "--negative-control-android-device-lab-d2d-queue-is-file-preflight",
    "--negative-control-android-device-lab-digest-artifact-file-metadata-failure",
    "--negative-control-android-device-lab-direct-helper-slot-secret-paths",
    "--negative-control-android-device-lab-direct-helper-slot-path-aliases",
    "--negative-control-android-device-lab-direct-symlink-artifact-slot-secret-paths",
    "--negative-control-android-device-lab-direct-hardlink-artifact-slot-secret-paths",
    "--negative-control-android-device-lab-direct-regular-artifact-slot-secret-paths",
    "--negative-control-android-device-lab-discover-slots-is-dir-preflight",
    "--negative-control-android-device-lab-discover-slots-entry-metadata-failure",
    "--negative-control-android-device-lab-duplicate-binding-zero-digest",
    "--negative-control-android-device-lab-duplicate-json-keys",
    "--negative-control-android-device-lab-hardlink-artifacts",
    "--negative-control-android-device-lab-hardlink-artifact-metadata-failure",
    "--negative-control-android-device-lab-hardlink-artifact-directory-exists-preflight",
    "--negative-control-android-device-lab-incomplete-slot-coverage",
    "--negative-control-android-device-lab-instrumentation-harness",
    "--negative-control-android-device-lab-json-load-ancestor",
    "--negative-control-android-device-lab-json-load-direct-secret-paths",
    "--negative-control-android-device-lab-json-load-direct-control-paths",
    "--negative-control-android-device-lab-json-load-direct-path-aliases",
    "--negative-control-android-device-lab-json-load-file-metadata-failure",
    "--negative-control-android-device-lab-json-load-size-limit",
    "--negative-control-android-device-lab-json-load-read-failure",
    "--negative-control-android-device-lab-json-load-open-path-binding",
    "--negative-control-android-device-lab-json-output-aliases",
    "--negative-control-android-device-lab-json-output-direct-secret-paths",
    "--negative-control-android-device-lab-json-output-direct-control-paths",
    "--negative-control-android-device-lab-json-output-direct-path-aliases",
    "--negative-control-android-device-lab-json-output-file-metadata-failure",
    "--negative-control-android-device-lab-json-output-hardlink-metadata-failure",
    "--negative-control-android-device-lab-json-output-parent-create-failure",
    "--negative-control-android-device-lab-json-output-parent-is-dir-preflight",
    "--negative-control-android-device-lab-json-output-parent-metadata-failure",
    "--negative-control-android-device-lab-json-output-post-create-parent-preflight",
    "--negative-control-android-device-lab-json-output-parent-sync-identity",
    "--negative-control-android-device-lab-json-output-post-write-preflight",
    "--negative-control-android-device-lab-json-output-published-cleanup-identity",
    "--negative-control-android-device-lab-json-output-published-cleanup-report",
    "--negative-control-android-device-lab-json-output-readback-verification",
    "--negative-control-android-device-lab-json-output-readback-failure",
    "--negative-control-android-device-lab-json-output-readback-size-limit",
    "--negative-control-android-device-lab-json-output-readback-open-path-binding",
    "--negative-control-android-device-lab-json-output-size-limit",
    "--negative-control-android-device-lab-json-output-strict-json-write",
    "--negative-control-android-device-lab-json-output-temp-cleanup-failure",
    "--negative-control-android-device-lab-json-output-temp-cleanup-identity",
    "--negative-control-android-device-lab-json-output-write-failure",
    "--negative-control-android-device-lab-main-root-exists-preflight",
    "--negative-control-android-device-lab-manifest-artifact-digest-preflight",
    "--negative-control-android-device-lab-manifest-artifact-open-path-binding",
    "--negative-control-android-device-lab-manifest-artifact-read-failure",
    "--negative-control-android-device-lab-manifest-artifact-size-limit",
    "--negative-control-android-device-lab-manifest-file-metadata-failure",
    "--negative-control-android-device-lab-manifest-file-shape-terminal",
    "--negative-control-android-device-lab-manifest-hardlink",
    "--negative-control-android-device-lab-manifest-hardlink-metadata-failure",
    "--negative-control-android-device-lab-manifest-open-path-binding",
    "--negative-control-android-device-lab-manifest-parse-direct-slot-secret-paths",
    "--negative-control-android-device-lab-manifest-read-failure",
    "--negative-control-android-device-lab-manifest-size-limit",
    "--negative-control-android-device-lab-manifest-slot-ancestor-symlink",
    "--negative-control-android-device-lab-manifest-slot-metadata-failure",
    "--negative-control-android-device-lab-manifest-slot-root-symlink",
    "--negative-control-android-device-lab-manifest-verify-direct-slot-secret-paths",
    "--negative-control-android-device-lab-manifest-verify-symlink-directory",
    "--negative-control-android-device-lab-slot-files-direct-root-shape",
    "--negative-control-android-device-lab-slot-files-root-metadata-failure",
    "--negative-control-android-device-lab-slot-files-direct-secret-paths",
    "--negative-control-android-device-lab-slot-files-direct-ancestor-symlink",
    "--negative-control-android-device-lab-slot-files-direct-symlink-directory",
    "--negative-control-android-device-lab-slot-files-directory-metadata-failure",
    "--negative-control-android-device-lab-slot-top-level-listing-failure",
    "--negative-control-android-device-lab-slot-files-artifact-metadata-failure",
    "--negative-control-android-device-lab-slot-dir-symlink",
    "--negative-control-android-device-lab-slot-metadata-failure",
    "--negative-control-android-device-lab-slot-parent-symlink",
    "--negative-control-android-device-lab-slot-parent-metadata-failure",
    "--negative-control-android-device-lab-slot-ancestor-symlink",
    "--negative-control-android-device-lab-slot-directory-traversal-failure",
    "--negative-control-android-device-lab-slot-regular-file-metadata-failure",
    "--negative-control-android-device-lab-slot-regular-file-exists-preflight",
    "--negative-control-android-device-lab-slot-directory-metadata-failure",
    "--negative-control-android-device-lab-slot-directory-exists-preflight",
    "--negative-control-android-device-lab-slot-artifact-file-metadata-failure",
    "--negative-control-android-device-lab-slot-artifact-symlink-preflight",
    "--negative-control-android-device-lab-slot-id-safety",
    "--negative-control-android-device-lab-slot-name-safety",
    "--negative-control-android-device-lab-slot-assembler-signature-required",
    "--negative-control-android-device-lab-slot-assembler-family-override-binding",
    "--negative-control-android-device-lab-slot-assembler-device-identity-fields",
    "--negative-control-android-device-lab-slot-assembler-harness-canonical",
    "--negative-control-android-device-lab-slot-assembler-report-app-package-binding",
    "--negative-control-android-device-lab-slot-assembler-result-closed-schema",
    "--negative-control-android-device-lab-slot-assembler-report-closed-schema",
    "--negative-control-android-device-lab-slot-assembler-report-verification-closed-schema",
    "--negative-control-android-device-lab-slot-assembler-report-schema",
    "--negative-control-android-device-lab-slot-assembler-report-verifier",
    "--negative-control-android-device-lab-slot-assembler-d2d-closed-schema",
    "--negative-control-android-device-lab-slot-assembler-wallet-closed-schema",
    "--negative-control-android-device-lab-slot-assembler-d2d-schema",
    "--negative-control-android-device-lab-slot-assembler-wallet-schema",
    "--negative-control-android-device-lab-slot-assembler-d2d-semantic-validation",
    "--negative-control-android-device-lab-slot-assembler-wallet-semantic-validation",
    "--negative-control-android-device-lab-slot-assembler-required-artifact-validation",
    "--negative-control-android-device-lab-slot-assembler-report-level-binding",
    "--negative-control-android-device-lab-slot-assembler-report-status-binding",
    "--negative-control-android-device-lab-slot-assembler-attestation-status-exactness",
    "--negative-control-android-device-lab-slot-assembler-source-open-binding",
    "--negative-control-android-device-lab-slot-assembler-root-path-aliases",
    "--negative-control-android-device-lab-slot-assembler-source-path-aliases",
    "--negative-control-android-device-lab-slot-assembler-copy-parent-sync-identity",
    "--negative-control-android-device-lab-slot-assembler-published-cleanup-identity",
    "--negative-control-android-device-lab-slot-assembler-published-cleanup-report",
    "--negative-control-android-device-lab-slot-assembler-copy-readback",
    "--negative-control-android-device-lab-slot-assembler-json-parent-sync-identity",
    "--negative-control-android-device-lab-slot-assembler-json-readback",
    "--negative-control-android-device-lab-slot-assembler-json-temp-cleanup-identity",
    "--negative-control-android-device-lab-slot-assembler-publish-root-identity",
    "--negative-control-android-device-lab-slot-assembler-publish-stage-identity",
    "--negative-control-android-device-lab-slot-assembler-temp-cleanup-identity",
    "--negative-control-android-device-lab-slot-assembler-temp-cleanup-report",
    "--negative-control-android-device-lab-test-workflow",
    "--negative-control-android-device-lab-wallet-integrity",
    "--negative-control-android-device-lab-unique-bindings",
    "--negative-control-android-device-lab-summary",
    "--negative-control-android-device-lab-summary-complete-evidence",
    "--negative-control-android-device-lab-summary-trusted-signer-binding",
    "--negative-control-android-device-lab-summary-zero-trusted-signer-digest",
    "--negative-control-android-device-lab-trusted-signer-map-path-type",
    "--negative-control-android-device-lab-trusted-signer-map-container",
    "--negative-control-android-device-lab-trusted-signer-map-mixed-key-sort",
    "--negative-control-android-device-lab-symlink-artifacts",
    "--negative-control-android-device-lab-symlink-artifact-leaf-metadata-failure",
    "--negative-control-android-device-lab-symlink-artifact-directory-metadata-failure",
    "--negative-control-android-device-lab-symlink-artifact-nested-metadata-failure",
    "--negative-control-android-device-lab-telemetry-closed-schema",
    "--negative-control-android-device-lab-telemetry-identity-exactness",
    "--negative-control-android-device-lab-telemetry-app-package-binding",
    "--negative-control-android-device-lab-status-event-closed-schema",
    "--negative-control-android-device-lab-status-value-closed-schema",
    "--negative-control-android-device-lab-status-slot-binding-required",
    "--negative-control-android-device-lab-transcript-artifact-digest-preflight",
    "--negative-control-android-device-lab-staged-bytes-open-path-binding",
    "--negative-control-android-device-lab-staged-bytes-hardlink-readback",
    "--negative-control-android-device-matrix",
    "--negative-control-android-signed-evidence-freshness-report",
    "--negative-control-android-signed-evidence-timestamp-raw",
    "--negative-control-android-signed-evidence-summary-partial-identity",
    "--negative-control-android-signed-evidence-summary-partial-artifact-binding",
    "--negative-control-android-signed-evidence-summary-partial-core-binding",
    "--negative-control-android-signed-evidence-summary-incomplete-entry",
    "--negative-control-android-signed-evidence-summary-slot-id",
    "--negative-control-android-slot-summary-incomplete-kagemusha",
    "--negative-control-android-duplicate-bindings-incomplete-slot-summary",
    "--negative-control-android-device-lab-metadata-artifact-digest-preflight",
    "--negative-control-android-device-lab-metadata-artifact-open-path-binding",
    "--negative-control-android-device-lab-metadata-artifact-read-failure",
    "--negative-control-android-device-lab-metadata-artifact-size-limit",
    "--negative-control-android-device-lab-minimum-os",
    "--negative-control-android-device-lab-nonfinite-json-constants",
    "--negative-control-android-device-lab-pending-queue-shape",
    "--negative-control-android-device-lab-pending-queue-closed-schema",
    "--negative-control-android-device-lab-pending-queue-empty-after-handoff",
    "--negative-control-android-device-lab-physical-device",
    "--negative-control-android-device-lab-private-key-ancestors",
    "--negative-control-android-device-lab-private-key-file-metadata-failure",
    "--negative-control-android-device-lab-private-key-hardlink-metadata-failure",
    "--negative-control-android-device-lab-private-key-missing-before-openssl",
    "--negative-control-android-device-lab-private-key-path-before-openssl",
    "--negative-control-android-device-lab-private-key-regular-file-before-openssl",
    "--negative-control-android-device-lab-private-public-pair-preserves-key-path-errors",
    "--negative-control-android-device-lab-production-claim-binding",
    "--negative-control-android-device-lab-public-key-file-metadata-failure",
    "--negative-control-android-device-lab-public-key-hardlink-metadata-failure",
    "--negative-control-android-device-lab-public-key-missing-before-openssl",
    "--negative-control-android-device-lab-public-key-openssl-invalid-key",
    "--negative-control-android-device-lab-public-key-openssl-spawn-failure",
    "--negative-control-android-device-lab-public-key-path-before-openssl",
    "--negative-control-android-device-lab-public-key-regular-file-before-openssl",
    "--negative-control-android-attestation-report-challenge-canonical",
    "--negative-control-android-attestation-report-chain-path-canonical",
    "--negative-control-android-attestation-report-chain-source-path-aliases",
    "--negative-control-android-attestation-report-harness-source-path-aliases",
    "--negative-control-android-attestation-report-slot-id-canonical",
    "--negative-control-android-attestation-report-identity-canonical",
    "--negative-control-android-attestation-report-strongbox-level-canonical",
    "--negative-control-android-attestation-report-chain-length-binding",
    "--negative-control-android-device-lab-zero-sha256-placeholders",
    "--negative-control-android-device-lab-source-zero-sha256-placeholders",
    "--negative-control-android-device-lab-apk-code-path-digest-exactness",
    "--negative-control-android-device-lab-release-apk-binding",
    "--negative-control-android-device-lab-signed-harness-result",
    "--negative-control-android-device-lab-signed-evidence-path-root",
    "--negative-control-android-device-lab-signed-evidence-path-canonical",
    "--negative-control-android-device-lab-signed-device-identity-binding",
    "--negative-control-android-device-lab-signed-artifact-schema",
    "--negative-control-android-device-lab-signed-evidence-artifact-digest-preflight",
    "--negative-control-android-device-lab-signed-evidence-artifact-size-limit",
    "--negative-control-android-device-lab-signed-evidence-artifact-is-file-preflight",
    "--negative-control-android-device-lab-signed-evidence-artifact-read-failure",
    "--negative-control-android-device-lab-signed-evidence-artifact-open-path-binding",
    "--negative-control-android-device-lab-signature-verify",
    "--negative-control-android-device-lab-signature-verify-staging-write-failure",
    "--negative-control-android-device-lab-signature-verify-tempdir-failure",
    "--negative-control-android-device-lab-signature-verify-spawn-failure",
    "--negative-control-android-device-lab-signed-evidence-canonical-payload-strict-json",
    "--negative-control-android-device-lab-signer-key-files",
    "--negative-control-android-device-lab-signer-key-ancestors",
    "--negative-control-android-device-lab-signature-verify-key-path-before-openssl",
    "--negative-control-android-device-lab-signer-key-secret-paths",
    "--negative-control-android-device-lab-signing-helper",
    "--negative-control-android-device-lab-signing-helper-canonical-payload-strict-json",
    "--negative-control-android-device-lab-signing-helper-signature-read-failure",
    "--negative-control-android-device-lab-signing-helper-signature-open-path-binding",
    "--negative-control-android-device-lab-signing-helper-signature-output-hardlink",
    "--negative-control-android-device-lab-signing-helper-signature-output-read-limit",
    "--negative-control-android-device-lab-signing-helper-signature-shape",
    "--negative-control-android-device-lab-signing-helper-signature-staging-write-failure",
    "--negative-control-android-device-lab-signing-helper-signature-tempdir-failure",
    "--negative-control-android-device-lab-signing-helper-signature-spawn-failure",
    "--negative-control-android-device-lab-signing-helper-signature-invalid-private-key",
    "--negative-control-android-device-lab-signing-helper-cli-secret-paths",
    "--negative-control-android-device-lab-signing-helper-dangling-output-alias",
    "--negative-control-android-device-lab-signing-helper-direct-manifest-shape",
    "--negative-control-android-device-lab-signing-helper-slot-listing-failure",
    "--negative-control-android-device-lab-signing-helper-slot-metadata-failure",
    "--negative-control-android-device-lab-signing-helper-slot-parent-metadata-failure",
    "--negative-control-android-device-lab-signing-helper-slot-artifact-digest-preflight",
    "--negative-control-android-device-lab-signing-helper-slot-artifact-size-limit",
    "--negative-control-android-device-lab-signing-helper-slot-artifact-hardlink-metadata-failure",
    "--negative-control-android-device-lab-signing-helper-slot-artifact-file-metadata-failure",
    "--negative-control-android-device-lab-signing-helper-slot-artifact-read-failure",
    "--negative-control-android-device-lab-signing-helper-slot-artifact-open-path-binding",
    "--negative-control-android-device-lab-signing-helper-direct-manifest-slot-secret-paths",
    "--negative-control-android-device-lab-signing-helper-direct-output-secret-paths",
    "--negative-control-android-device-lab-signing-helper-direct-slot-path-aliases",
    "--negative-control-android-device-lab-signing-helper-direct-slot-secret-paths",
    "--negative-control-android-device-lab-signing-helper-json-output-path-aliases",
    "--negative-control-android-device-lab-signing-helper-json-write-failure",
    "--negative-control-android-device-lab-signing-helper-manifest-secret-paths",
    "--negative-control-android-device-lab-signing-helper-manifest-size-limit",
    "--negative-control-android-device-lab-signing-helper-manifest-write",
    "--negative-control-android-device-lab-signing-helper-metadata-preflight",
    "--negative-control-android-device-lab-signing-helper-artifact-digests-preflight",
    "--negative-control-android-device-lab-signing-helper-output-write",
    "--negative-control-android-device-lab-signing-helper-output-strict-json-write",
    "--negative-control-android-device-lab-signing-helper-output-size-limit",
    "--negative-control-android-device-lab-signing-helper-output-ancestor",
    "--negative-control-android-device-lab-signing-helper-output-parent-is-dir-preflight",
    "--negative-control-android-device-lab-signing-helper-output-parent-metadata-failure",
    "--negative-control-android-device-lab-signing-helper-output-parent-create-failure",
    "--negative-control-android-device-lab-signing-helper-output-post-create-parent-preflight",
    "--negative-control-android-device-lab-signing-helper-output-parent-sync-identity",
    "--negative-control-android-device-lab-signing-helper-published-cleanup-identity",
    "--negative-control-android-device-lab-signing-helper-output-resolve-failure",
    "--negative-control-android-device-lab-signing-helper-output-file-metadata-failure",
    "--negative-control-android-device-lab-signing-helper-output-hardlink-metadata-failure",
    "--negative-control-android-device-lab-signing-helper-output-digest-preflight",
    "--negative-control-android-device-lab-signing-helper-output-digest-parent-missing",
    "--negative-control-android-device-lab-signing-helper-output-digest-leaf-missing",
    "--negative-control-android-device-lab-signing-helper-output-digest-file-metadata-failure",
    "--negative-control-android-device-lab-signing-helper-output-digest-hardlink-metadata-failure",
    "--negative-control-android-device-lab-signing-helper-output-digest-size-limit",
    "--negative-control-android-device-lab-signing-helper-output-digest-read-failure",
    "--negative-control-android-device-lab-signing-helper-output-digest-open-path-binding",
    "--negative-control-android-device-lab-signing-helper-post-write-preflight",
    "--negative-control-android-device-lab-signing-helper-readback-verification",
    "--negative-control-android-device-lab-signing-helper-readback-failure",
    "--negative-control-android-device-lab-signing-helper-temp-cleanup-failure",
    "--negative-control-android-device-lab-signing-helper-temp-cleanup-identity",
    "--negative-control-android-device-lab-signing-helper-text-size-limit",
    "--negative-control-android-device-lab-signing-helper-text-write-failure",
    "--negative-control-android-device-lab-regular-file-artifacts",
    "--negative-control-android-device-lab-required-artifacts",
    "--negative-control-android-device-lab-required-artifact-is-file-preflight",
    "--negative-control-android-device-lab-required-status-is-file-preflight",
    "--negative-control-android-device-lab-required-runtime-log-is-file-preflight",
    "--negative-control-android-device-lab-required-artifact-shape",
    "--negative-control-android-device-lab-required-artifact-metadata-failure",
    "--negative-control-android-device-lab-required-artifact-content",
    "--negative-control-android-device-lab-required-text-artifact-read-preflight",
    "--negative-control-android-device-lab-relative-ancestor-is-symlink-preflight",
    "--negative-control-android-device-lab-scan-slot-expected-dir-is-dir-preflight",
    "--negative-control-android-device-lab-scan-slot-artifact-count-is-file-preflight",
    "--negative-control-android-device-lab-scan-slot-sha-is-file-preflight",
    "--negative-control-android-device-lab-secret-redaction",
    "--negative-control-android-device-lab-root-direct-secret-paths",
    "--negative-control-android-device-lab-root-direct-control-paths",
    "--negative-control-android-device-lab-root-direct-path-aliases",
    "--negative-control-android-device-lab-root-metadata-failure",
    "--negative-control-android-device-lab-rollup-root-exists-preflight",
    "--negative-control-android-device-lab-root-symlink",
    "--negative-control-android-device-lab-root-ancestor-symlink",
    "--negative-control-android-device-lab-root-discovery-read-failure",
    "--negative-control-android-device-lab-scanner-harness-canonical",
    "--negative-control-android-device-lab-root-summary-label-exactness",
    "--negative-control-android-device-lab-telemetry-suite-exactness",
    "--negative-control-android-device-lab-size-cap-constant-exactness",
    "--negative-control-android-device-lab-doc-install-marker-exactness",
    "--negative-control-android-device-lab-raw-command-exact",
    "--negative-control-android-device-lab-raw-command-marker-specificity",
    "--negative-control-android-device-lab-raw-command-constant-exactness",
    "--negative-control-android-device-lab-raw-command-marker-tuple-exactness",
    "--negative-control-android-device-matrix-attestation-result-doc-exactness",
    "--negative-control-android-device-matrix-physical-attestation-doc-exactness",
    "--negative-control-android-device-matrix-generated-at-utc-doc-exactness",
    "--negative-control-android-device-matrix-signed-evidence-path-doc-exactness",
    "--negative-control-android-device-lab-raw-puller-blank-serial",
    "--negative-control-android-device-lab-raw-puller-overwrite",
    "--negative-control-android-device-lab-raw-puller-install-no-overwrite",
    "--negative-control-android-device-lab-raw-puller-install-top-level",
    "--negative-control-android-device-lab-raw-puller-install-parent-sync",
    "--negative-control-android-device-lab-raw-puller-install-directory-identity",
    "--negative-control-android-device-lab-raw-puller-install-sync-identity",
    "--negative-control-android-device-lab-raw-puller-install-cleanup-identity",
    "--negative-control-android-device-lab-raw-puller-install-cleanup-report",
    "--negative-control-android-device-lab-raw-puller-temp-cleanup-identity",
    "--negative-control-android-device-lab-raw-puller-temp-cleanup-report",
    "--negative-control-android-device-lab-raw-puller-install-rename-dir-fd",
    "--negative-control-android-device-lab-raw-puller-install-output-root-identity",
    "--negative-control-android-device-lab-raw-puller-install-cleanup-dir-fd",
    "--negative-control-android-device-lab-raw-puller-install-slot-entry-dir-fd",
    "--negative-control-android-device-lab-raw-puller-path-aliases",
    "--negative-control-android-device-lab-raw-puller-allowed-artifacts",
    "--negative-control-android-device-lab-raw-puller-directory-collision",
    "--negative-control-android-device-lab-raw-puller-entry-cap",
    "--negative-control-android-device-lab-raw-puller-summary-strict-json",
    "--negative-control-android-device-lab-raw-puller-summary-size-limit",
    "--negative-control-android-device-lab-raw-puller-summary-parent-sync",
    "--negative-control-android-device-lab-raw-puller-summary-parent-identity",
    "--negative-control-android-device-lab-raw-puller-summary-readback-symlink",
    "--negative-control-android-device-lab-raw-puller-summary-readback-hardlink",
    "--negative-control-android-device-lab-raw-puller-summary-readback-identity",
    "--negative-control-android-device-lab-raw-puller-summary-private-permissions",
    "--negative-control-android-device-lab-raw-puller-summary-temp-cleanup-identity",
    "--negative-control-android-device-lab-raw-puller-published-cleanup-identity",
    "--negative-control-android-device-lab-raw-puller-summary-digest-open-path",
    "--negative-control-android-device-lab-raw-puller-summary-digest-inventory",
    "--negative-control-android-device-lab-raw-harness-result",
    "--negative-control-android-device-lab-raw-puller-json-slot-binding",
    "--negative-control-android-device-lab-raw-puller-d2d-offline",
    "--negative-control-android-device-lab-raw-puller-wallet-rollback",
    "--negative-control-android-device-lab-raw-puller-status-failure",
    "--negative-control-android-device-lab-raw-puller-runtime-failure-marker",
    "--negative-control-android-device-lab-raw-puller-harness-challenge",
    "--negative-control-android-device-lab-raw-puller-harness-strongbox",
    "--negative-control-android-device-lab-raw-puller-harness-chain-length",
    "--negative-control-android-device-lab-raw-puller-harness-canonical",
    "--negative-control-android-device-lab-raw-puller-challenge-file-canonical",
    "--negative-control-android-device-lab-raw-puller-latest-slot-canonical",
    "--negative-control-android-device-lab-raw-puller-latest-query-canonical",
    "--negative-control-android-device-lab-raw-puller-latest-write-parent-identity",
    "--negative-control-android-device-lab-raw-puller-latest-write-readback-symlink",
    "--negative-control-android-device-lab-raw-puller-latest-write-readback-hardlink",
    "--negative-control-android-device-lab-raw-puller-latest-write-readback-identity",
    "--negative-control-android-device-lab-raw-puller-latest-write-private-permissions",
    "--negative-control-android-device-lab-raw-puller-latest-write-temp-cleanup-identity",
    "--negative-control-android-device-lab-raw-puller-result-slot-required",
    "--negative-control-android-device-lab-raw-puller-result-chain-digest-required",
    "--negative-control-android-device-lab-raw-puller-result-challenge-digest-required",
    "--negative-control-android-device-lab-raw-puller-result-closed-schema",
    "--negative-control-android-device-lab-raw-puller-result-identity-strings",
    "--negative-control-android-device-lab-raw-puller-result-sdk-digests",
    "--negative-control-android-device-lab-raw-puller-result-strongbox-levels",
    "--negative-control-android-device-lab-raw-puller-private-permissions",
    "--negative-control-android-device-lab-attestation-report-writer-physical-device",
    "--negative-control-android-device-lab-attestation-report-writer-parent-sync-identity",
    "--negative-control-android-device-lab-attestation-report-writer-published-cleanup-identity",
    "--negative-control-android-device-lab-attestation-report-writer-temp-cleanup-failure",
    "--negative-control-android-device-lab-attestation-report-writer-temp-cleanup-identity",
    "--negative-control-android-device-lab-attestation-report-writer-private-permissions",
    "--negative-control-android-device-lab-slot-assembler-private-permissions",
    "--negative-control-android-device-lab-slot-assembler-source-identity-fallback",
    "--negative-control-android-device-lab-d2d-transport-matrix",
    "--negative-control-android-release-bundle-d2d-declaration-binding",
    "--negative-control-release-bundle-android-d2d-transport-list-shape",
    "--negative-control-release-bundle-android-d2d-transcript-binding-shape",
    "--negative-control-release-bundle-summary-drift",
    "--negative-control-release-bundle-top-level-evidence-path",
    "--negative-control-release-bundle-top-level-evidence-binding",
    "--negative-control-release-bundle-abi7-fixture-manifest-digest-binding",
    "--negative-control-release-bundle-abi7-archive-fixture-digest-binding",
    "--negative-control-release-bundle-abi7-fixture-digest-shape",
    "--negative-control-release-bundle-abi7-section-value-binding",
    "--negative-control-release-bundle-abi7-section-shape",
    "--negative-control-release-bundle-abi6-section-value-binding",
    "--negative-control-release-bundle-abi6-nested-value-binding",
    "--negative-control-release-bundle-abi6-section-shape",
    "--negative-control-release-bundle-section-evidence-binding",
    "--negative-control-release-bundle-compact-generator-log-artifact-binding",
    "--negative-control-abi-fixture-integer-scalars",
    "--negative-control-release-bundle-summary-shape",
    "--negative-control-release-bundle-summary-section-schema",
    "--negative-control-release-bundle-android-signed-evidence-summary-schema",
    "--negative-control-release-bundle-android-slot-entry-shape",
    "--negative-control-release-bundle-android-signed-evidence-entry-shape",
    "--negative-control-release-bundle-android-summary-list-shape",
    "--negative-control-release-bundle-android-manifest-list-shape",
    "--negative-control-release-bundle-android-slot-errors-shape",
    "--negative-control-release-bundle-android-slot-present-shape",
    "--negative-control-release-bundle-android-slot-file-counts-shape",
    "--negative-control-release-bundle-android-duplicate-binding-list-shape",
    "--negative-control-release-bundle-android-duplicate-binding-entry-shape",
    "--negative-control-release-bundle-android-duplicate-binding-entry-schema",
    "--negative-control-release-bundle-android-duplicate-binding-slot-binding",
    "--negative-control-release-bundle-android-duplicate-binding-value-binding",
    "--negative-control-release-bundle-android-duplicate-binding-value-inventory",
    "--negative-control-release-bundle-blocked-manifest-trusted-signer-sanitization",
    "--negative-control-release-bundle-evidence-inventory-schema",
    "--negative-control-release-bundle-evidence-inventory-keysets",
    "--negative-control-release-bundle-section-schema",
    "--negative-control-release-bundle-android-manifest-schema",
    "--negative-control-release-bundle-artifact-inventory",
    "--negative-control-release-bundle-android-slot-artifact-inventory",
    "--negative-control-release-bundle-compact-placeholder-inventory",
    "--negative-control-release-bundle-compact-generator-log-inventory",
    "--negative-control-release-bundle-evidence-entry-nonempty",
    "--negative-control-release-bundle-evidence-entry-open-path-binding",
    "--negative-control-release-bundle-json-input-open-path-binding",
    "--negative-control-release-bundle-local-json-size-limit",
    "--negative-control-release-bundle-digest-open-path-binding",
    "--negative-control-release-bundle-atomic-output",
    "--negative-control-release-bundle-temp-cleanup-failure",
    "--negative-control-release-bundle-temp-cleanup-identity",
    "--negative-control-release-bundle-strict-json-write",
    "--negative-control-release-bundle-output-size-limit",
    "--negative-control-release-bundle-output-readback-failure",
    "--negative-control-release-bundle-output-readback-size-limit",
    "--negative-control-release-bundle-output-readback-open-path-binding",
    "--negative-control-release-bundle-output-private-permissions",
    "--negative-control-release-bundle-output-parent-sync-identity",
    "--negative-control-release-bundle-output-published-cleanup-identity",
    "--negative-control-release-bundle-output-post-write-preflight",
    "--negative-control-release-bundle-control-path-preflight",
    "--negative-control-release-bundle-input-path-preflight",
    "--negative-control-release-bundle-scan-preflight",
    "--negative-control-release-bundle-output-overwrite",
    "--negative-control-release-bundle-verify-existing",
    "--negative-control-release-bundle-verify-existing-preflight",
    "--negative-control-release-bundle-verify-existing-evidence-path-shape",
    "--negative-control-release-bundle-android-summary-binding",
    "--negative-control-release-bundle-android-signed-evidence-summary-binding",
    "--negative-control-release-bundle-android-signed-evidence-binding",
    "--negative-control-release-bundle-android-signed-evidence-identity",
    "--negative-control-release-bundle-android-slot-summary-identity",
    "--negative-control-release-bundle-android-signed-evidence-identity-drift",
    "--negative-control-release-bundle-android-slot-identity-drift",
    "--negative-control-release-bundle-manifest-android-signed-evidence-identity-binding",
    "--negative-control-release-bundle-android-signer-binding",
    "--negative-control-release-bundle-android-slot-artifact-binding",
    "--negative-control-release-bundle-manifest-shape",
    "--negative-control-release-bundle-cli-missing-evidence-summary",
    "--negative-control-release-bundle-ready-summary-top-level-blockers",
    "--negative-control-release-bundle-ready-manifest-top-level-blockers",
    "--negative-control-kagemusha-readiness-cli-external-blockers",
    "--negative-control-kagemusha-readiness-summary-output-private-permissions",
    "--negative-control-abi7-fixture-closed-schema",
    "--negative-control-abi7-fixture-nested-manifest-closed-schema",
    "--negative-control-abi7-fixture-nested-object-shape",
    "--negative-control-abi7-fixture-json-object-shape",
    "--negative-control-abi7-archive-fixture-entry-shape",
    "--negative-control-abi7-archive-fixture-field-shapes",
    "--negative-control-abi7-archive-fixture-canonical-base64",
    "--negative-control-abi7-fixture-operation-shape",
    "--negative-control-abi7-fixture-archive-reference-shape",
    "--negative-control-abi7-fixture-strict-json",
    "--negative-control-abi7-fixture-json-size-limit",
    "--negative-control-abi7-fixture-file-aliases",
    "--negative-control-abi7-fixture-race-and-ancestor-aliases",
    "--negative-control-abi7-fixture-manifest-value-binding",
    "--negative-control-abi7-archive-fixture-value-binding",
    "--negative-control-abi7-fixture-unreadable-json",
    "--negative-control-abi7-fixture-operation-closed-schema",
    "--negative-control-abi7-fixture-duplicate-archive",
    "--negative-control-lineage-key-release-source-marker-aliases",
    "--negative-control-lineage-key-release-source-marker-non-utf8-read",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_production_readiness.sh",
    expectedModes,
    "Kagemusha production readiness guard",
  );
  for (const mode of expectedModes) {
    assert.ok(
      readiness.includes(`ci/check_kagemusha_production_readiness.sh ${mode}`),
      `production readiness workflow requirements must include ${mode}`,
    );
    assert.ok(readiness.includes(`if mode == "${mode}":`), `production readiness guard must implement ${mode}`);
  }

  const readinessBranch = (mode) => {
    const start = readiness.indexOf(`if mode == "${mode}":`);
    assert.notEqual(start, -1, `missing readiness branch ${mode}`);
    const end = readiness.indexOf("\nif mode ==", start + 1);
    return readiness.slice(start, end === -1 ? readiness.length : end);
  };

  assertContainsAll(
    readiness,
    ["test_lineage_verifier_witness_profile_matches_data_model_constant"],
    "Kagemusha readiness verifier witness profile guard",
  );
  assertContainsAll(
    readiness,
    [
      verifierWitnessProfile,
      "64-by-4 scalar coverage",
      "64-by-4 fixed-window Vesta verifier witness profile",
    ],
    "Kagemusha readiness verifier witness profile docs requirements",
  );
  assertContainsAll(
    readinessScript,
    [
      "EXPECTED_LINEAGE_VERIFIER_WITNESS_PROFILE = (",
      `"${verifierWitnessProfile}"`,
      '"verifier_witness_profile": EXPECTED_LINEAGE_VERIFIER_WITNESS_PROFILE',
    ],
    "Kagemusha readiness verifier witness profile source",
  );
  assertContainsAll(
    dataModel,
    [
      "pub const KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1: &str =\n" +
        `    "${verifierWitnessProfile}";`,
    ],
    "Kagemusha data-model verifier witness profile constant",
  );
  assert.match(
    readinessTests,
    /test_lineage_proof_evidence_drift_blocks_rollup_section[\s\S]*vesta-recursive-fixed-window-85x3[\s\S]*lineage_proof_evidence_verifier_witness_profile/u,
    "Kagemusha readiness tests must reject stale verifier witness profile evidence",
  );

  const branchSpecs = [
    [
      "--negative-control-abi6-manifest",
      /manifest\["operation_count"\] = 8/u,
      "ABI-6 manifest operation count",
    ],
    [
      "--negative-control-abi6-manifest-direct-invalid-json",
      /text_overrides\["fixtures\/kagemusha_recursive_spend_abi6\/manifest\.json"\] = \([\s\S]*?\{"schema": /u,
      "ABI-6 manifest direct invalid JSON",
    ],
    [
      "--negative-control-abi6-manifest-direct-duplicate-json-key",
      /\{"schema": "first", "schema": "second"\}/u,
      "ABI-6 manifest direct duplicate JSON key",
    ],
    [
      "--negative-control-abi6-manifest-direct-nonfinite-json",
      /manifest\["ignored_nonfinite"\] = float\("nan"\)/u,
      "ABI-6 manifest direct non-finite JSON",
    ],
    [
      "--negative-control-abi6-manifest-direct-object-shape",
      /\["token=abi6-direct-object-secret"\]/u,
      "ABI-6 manifest direct object shape",
    ],
    [
      "--negative-control-abi6-manifest-direct-closed-schema",
      /manifest\["token=abi6-direct-top-secret"\] = "must stay hidden"/u,
      "ABI-6 manifest direct closed schema",
    ],
    [
      "--negative-control-abi6-manifest-direct-nested-closed-schema",
      /token=abi6-direct-archive-secret[\s\S]*?token=abi6-direct-payload-secret/u,
      "ABI-6 manifest direct nested closed schema",
    ],
    [
      "--negative-control-abi6-manifest-direct-nested-value-binding",
      /token=abi6-direct-archive-value-secret[\s\S]*?token=abi6-direct-payload-value-secret/u,
      "ABI-6 manifest direct nested value binding",
    ],
    [
      "--negative-control-abi6-manifest-direct-operation-value-binding",
      /token=abi6-direct-operation-value-secret[\s\S]*?token=abi6-direct-operation-kind-secret/u,
      "ABI-6 manifest direct operation value binding",
    ],
    [
      "--negative-control-abi6-manifest-integer-scalars",
      /manifest\["native_bridge_abi_version"\] = 6\.0[\s\S]*?manifest\["operation_count"\] = float\(len\(ABI6_SYMBOLS\)\)/u,
      "ABI-6 manifest integer scalar exactness",
    ],
    [
      "--negative-control-abi6-manifest-limit-integer-scalars",
      /manifest\["limits"\]\["compact_token_max_hops"\] = 64\.0/u,
      "ABI-6 manifest limit integer scalar exactness",
    ],
    [
      "--negative-control-abi6-manifest-direct-operation-shape",
      /manifest\["operations"\] = \{"token=abi6-direct-operation-secret": \[\]\}/u,
      "ABI-6 manifest direct operation shape",
    ],
    [
      "--negative-control-abi6-manifest-direct-limits-shape",
      /manifest\["limits"\] = "token=abi6-direct-limit-secret"/u,
      "ABI-6 manifest direct limits shape",
    ],
    [
      "--negative-control-abi6-manifest-direct-modes-shape",
      /manifest\["modes"\] = \["token=abi6-direct-mode-secret"\]/u,
      "ABI-6 manifest direct modes shape",
    ],
    [
      "--negative-control-abi6-manifest-operation-shape",
      /not isinstance\(operation, dict\)[\s\S]*?False/u,
      "ABI-6 manifest operation shape",
    ],
    [
      "--negative-control-abi6-manifest-closed-schema",
      /abi6_manifest_unexpected_field[\s\S]*?abi6_manifest_unchecked_field[\s\S]*?abi6_manifest_operation_unexpected_field[\s\S]*?abi6_manifest_operation_unchecked_field[\s\S]*?abi6_manifest_limit_unexpected_field[\s\S]*?abi6_manifest_limit_unchecked_field[\s\S]*?abi6_manifest_mode_unexpected_field[\s\S]*?abi6_manifest_mode_unchecked_field/u,
      "ABI-6 manifest closed schema",
    ],
    [
      "--negative-control-abi6-manifest-nested-closed-schema",
      /abi6_manifest_archive_fixture_unexpected_field[\s\S]*?abi6_manifest_archive_fixture_unchecked_field[\s\S]*?abi6_manifest_proof_circuit_ids_unexpected_field[\s\S]*?abi6_manifest_proof_circuit_ids_unchecked_field[\s\S]*?abi6_manifest_domains_unexpected_field[\s\S]*?abi6_manifest_domains_unchecked_field[\s\S]*?abi6_manifest_hop_policy_unexpected_field[\s\S]*?abi6_manifest_hop_policy_unchecked_field[\s\S]*?abi6_manifest_hop_policy_entry_unexpected_field[\s\S]*?abi6_manifest_hop_policy_entry_unchecked_field[\s\S]*?abi6_manifest_payload_benchmarks_unexpected_field[\s\S]*?abi6_manifest_payload_benchmarks_unchecked_field/u,
      "ABI-6 manifest nested closed schema",
    ],
    [
      "--negative-control-abi6-manifest-nested-shape",
      /abi6_manifest_archive_fixture_shape[\s\S]*?abi6_manifest_archive_fixture_accepts_array[\s\S]*?abi6_manifest_proof_circuit_ids_shape[\s\S]*?abi6_manifest_proof_circuit_ids_accepts_array[\s\S]*?abi6_manifest_domains_shape[\s\S]*?abi6_manifest_domains_accepts_array[\s\S]*?abi6_manifest_hop_policy_shape[\s\S]*?abi6_manifest_hop_policy_accepts_array[\s\S]*?abi6_manifest_hop_policy_entry_shape[\s\S]*?abi6_manifest_hop_policy_entry_accepts_string[\s\S]*?abi6_manifest_payload_benchmarks_shape[\s\S]*?abi6_manifest_payload_benchmarks_accepts_array/u,
      "ABI-6 manifest nested shape",
    ],
    [
      "--negative-control-abi6-manifest-nested-value-binding",
      /"abi6_manifest_fixture_kind",[\s\S]*?"abi6_manifest_fixture_kind_disabled",[\s\S]*?"abi6_manifest_archive_fixture",[\s\S]*?"abi6_manifest_archive_fixture_disabled",[\s\S]*?"abi6_manifest_proof_circuit_ids",[\s\S]*?"abi6_manifest_proof_circuit_ids_disabled",[\s\S]*?"abi6_manifest_domains",[\s\S]*?"abi6_manifest_domains_disabled",[\s\S]*?"abi6_manifest_hop_policy",[\s\S]*?"abi6_manifest_hop_policy_disabled",[\s\S]*?"abi6_manifest_payload_benchmarks",[\s\S]*?"abi6_manifest_payload_benchmarks_disabled",/u,
      "ABI-6 manifest nested value binding",
    ],
    [
      "--negative-control-abi6-manifest-file-aliases",
      /abi6_manifest_file_shape[\s\S]*?abi6_manifest_file_alias_allowed/u,
      "ABI-6 manifest file alias gate",
    ],
    [
      "--negative-control-abi6-manifest-ancestor-aliases",
      /release_json_ancestor_errors = device_lab\.validate_no_symlink_ancestors\([\s\S]*?release_json_ancestor_errors = _skip_release_json_ancestor_validation\(/u,
      "ABI-6 manifest ancestor alias gate",
    ],
    [
      "--negative-control-abi7-source-marker-file-aliases",
      /abi7_source_marker_file_shape[\s\S]*?abi7_source_marker_file_alias_allowed/u,
      "ABI-7 source marker file alias gate",
    ],
    [
      "--negative-control-compact-open",
      /multi-hop proving requires the append verifier batch to be composed into the compact proof[\s\S]*?multi-hop proving is enabled without the append verifier batch/u,
      "ABI-7 compact multi-hop fail-closed gate",
    ],
    [
      "--negative-control-abi7-core-contract-open",
      /public ABI-7 compact token one-hop shape preverification[\s\S]*?public ABI-7 compact token disabled shape preverification/u,
      "ABI-7 one-hop compact core function contract",
    ],
    [
      "--negative-control-abi7-one-hop-runtime-keygen-fallback",
      /missing compact one-hop proving key archive[\s\S]*?runtime-generated compact one-hop proving key archive accepted/u,
      "ABI-7 one-hop runtime keygen fallback",
    ],
    [
      "--negative-control-abi7-append-runtime-keygen-fallback",
      /missing compact append proving key archive[\s\S]*?runtime-generated compact append proving key archive accepted/u,
      "ABI-7 append runtime keygen fallback",
    ],
    [
      "--negative-control-abi7-bridge-unavailable-mapping",
      /BridgeError::KagemushaRecursiveCompactUnavailable[\s\S]*?BridgeError::KagemushaProve/u,
      "ABI-7 bridge unavailable mapping",
    ],
    [
      "--negative-control-abi7-offline-doc-one-hop-boundary",
      /ABI-7 recursive compact-token symbols now route one-hop[\s\S]*?ABI-7 recursive compact-token symbols are globally disabled/u,
      "ABI-7 offline doc one-hop compact boundary",
    ],
    [
      "--negative-control-offline-doc-evidence-filename-exactness",
      /`artifacts\/kagemusha\/lineage-proof-evidence\.json` and\\n[\s\S]*?""/u,
      "offline Kagemusha release-evidence filename exactness",
    ],
    [
      "--negative-control-offline-doc-compact-generator-log-exactness",
      /the captured `recursive-compact-key-artifacts\.log` stdout line from the\\n[\s\S]*?""/u,
      "offline Kagemusha compact generator-log prose exactness",
    ],
    [
      "--negative-control-offline-doc-release-bundle-output-exactness",
      /--out dist\/kagemusha-production-release-bundle\.json\\n[\s\S]*?""/u,
      "offline Kagemusha release-bundle output exactness",
    ],
    [
      "--negative-control-offline-doc-verifier-profile-exactness",
      /pallas-ipa-transparent-v1\/vesta-recursive-fixed-window-64x4[\s\S]*?pallas-ipa-transparent-v1\/vesta-recursive-fixed-window-85x3[\s\S]*?64-by-4 scalar coverage[\s\S]*?85-by-3 scalar coverage/u,
      "offline Kagemusha verifier-witness profile exactness",
    ],
    [
      "--negative-control-compact-key-release-tooling",
      /derive_halo2_ipa_kagemusha_recursive_compact_payment_token_proving_key_bytes[\s\S]*?derive_halo2_ipa_kagemusha_recursive_compact_payment_token_disabled/u,
      "ABI-7 compact key release tooling",
    ],
    [
      "--negative-control-compact-key-evidence",
      /compact_key_evidence_missing[\s\S]*?compact_key_evidence_optional/u,
      "ABI-7 compact key evidence required packet",
    ],
    [
      "--negative-control-compact-key-evidence-path-aliases",
      /compact_key_evidence_path=compact_key_evidence_path,[\s\S]*?compact_key_evidence_path=compact_key_evidence_path\.resolve\(\),/u,
      "ABI-7 compact key evidence path alias gate",
    ],
    [
      "--negative-control-compact-key-artifact-prefix-binding",
      /validate_compact_key_artifact_prefix\(artifact_prefix, artifact\)[\s\S]*?validate_compact_key_artifact_content\(artifact_path, artifact\)[\s\S]*?validate_compact_key_artifact_prefix\(artifact_prefix, artifact\)[\s\S]*?validate_compact_key_artifact_content\(path, artifact\)/u,
      "ABI-7 recursive compact key evidence artifact prefix binding",
    ],
    [
      "--negative-control-compact-key-artifact-size-binding",
      /_require_compact_key_artifact_size[\s\S]*?_compact_key_artifact_size_disabled/u,
      "ABI-7 recursive compact key artifact size binding",
    ],
    [
      "--negative-control-compact-key-evidence-json-size-limit",
      /max_bytes=MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES[\s\S]*?max_bytes=None/u,
      "ABI-7 recursive compact key evidence JSON size limit",
    ],
    [
      "--negative-control-compact-key-readiness-artifact-open-path-binding",
      /expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "ABI-7 recursive compact key readiness artifact open-path binding",
    ],
    [
      "--negative-control-compact-key-placeholder-artifacts",
      /must be generated key material, not a placeholder fixture[\s\S]*?may use placeholder fixture material/u,
      "ABI-7 recursive compact key placeholder artifact gate",
    ],
    [
      "--negative-control-kagemusha-readiness-cli-external-blockers",
      /test_cli_without_external_evidence_reports_all_release_blockers[\s\S]*?test_cli_without_external_evidence_allows_missing_release_blockers/u,
      "Kagemusha readiness CLI external evidence blocker coverage",
    ],
    [
      "--negative-control-compact-key-command-canonical",
      /must exactly match the canonical ABI-7 recursive compact keygen command string[\s\S]*?canonical compact key command spelling accepted/u,
      "ABI-7 compact key evidence canonical command gate",
    ],
    [
      "--negative-control-compact-key-generator-log-binding",
      /compact_key_evidence_generator_log_artifact_size[\s\S]*?compact_key_evidence_generator_log_unchecked_size/u,
      "ABI-7 compact key evidence generator log binding",
    ],
    [
      "--negative-control-compact-key-generator-log-digest-binding",
      /compact_key_evidence_generator_log_artifact_digest[\s\S]*?compact_key_evidence_generator_log_unchecked_digest/u,
      "ABI-7 recursive compact key generator log digest binding",
    ],
    [
      "--negative-control-compact-key-generator-log-size-limit",
      /max_bytes=MAX_COMPACT_KEY_GENERATOR_LOG_BYTES[\s\S]*?max_bytes=None/u,
      "ABI-7 recursive compact key generator log size limit",
    ],
    [
      "--negative-control-compact-key-generator-log-open-path-binding",
      /_sha256_text_file\([\s\S]*?ABI-7 recursive compact key generator log[\s\S]*?_sha256_text_file_unbound\(/u,
      "ABI-7 recursive compact key generator log open-path binding",
    ],
    [
      "--negative-control-compact-key-helper-validation-dir-create-failure",
      /artifact_dir\.mkdir\(mode=0o700, parents=True, exist_ok=True\)[\s\S]*?--artifact-dir could not be created for evidence validation[\s\S]*?artifact_dir\.mkdir\(mode=0o700, parents=True, exist_ok=True\)/u,
      "ABI-7 recursive compact key evidence helper validation dir create-failure gate",
    ],
    [
      "--negative-control-compact-key-helper-validation-strict-json-write",
      /recursive compact key evidence validation file is not strict JSON[\s\S]*?recursive compact key evidence validation file allows non-strict JSON/u,
      "ABI-7 recursive compact key evidence helper validation strict JSON gate",
    ],
    [
      "--negative-control-compact-key-helper-validation-temp-write-failure",
      /recursive compact key evidence validation file could not be written[\s\S]*?recursive compact key evidence validation file write failures ignored/u,
      "ABI-7 recursive compact key evidence helper validation temp write-failure gate",
    ],
    [
      "--negative-control-compact-key-helper-validation-temp-cleanup-after-write-failure",
      /errors\.extend\(_cleanup_validation_temp_output\(path, tmp_identity\)\)[\s\S]*?pass/u,
      "ABI-7 recursive compact key evidence helper validation temp cleanup after write-failure gate",
    ],
    [
      "--negative-control-compact-key-helper-validation-temp-cleanup-failure",
      /recursive compact key evidence validation file could not be removed[\s\S]*?recursive compact key evidence validation file cleanup failures ignored/u,
      "ABI-7 recursive compact key evidence helper validation temp cleanup-failure gate",
    ],
    [
      "--negative-control-compact-key-helper-validation-temp-cleanup-identity",
      /_file_identity\(validation_temp_stat\) != expected_identity[\s\S]*?False/u,
      "ABI-7 recursive compact key evidence helper validation temp cleanup identity",
    ],
    [
      "--negative-control-compact-key-helper-direct-artifact-dir-secret-paths",
      /secret_error = _secret_path_error\(str\(artifact_dir\), "--artifact-dir"\)[\s\S]*?return \[secret_error\][\s\S]*?def validate_artifact_dir_path/u,
      "ABI-7 recursive compact key evidence helper direct artifact-dir secret-path gate",
    ],
    [
      "--negative-control-compact-key-helper-direct-artifact-dir-metadata-failure",
      /artifact_dir\.lstat\(\)\.st_mode[\s\S]*?--artifact-dir metadata could not be read[\s\S]*?artifact_dir\.lstat\(\)\.st_mode/u,
      "ABI-7 recursive compact key evidence helper direct artifact-dir metadata failure gate",
    ],
    [
      "--negative-control-compact-key-helper-direct-hash-shape",
      /_validate_lineage_local_file_for_read\([\s\S]*?return None, file_errors[\s\S]*?expected_stat = path\.stat\(\)/u,
      "ABI-7 recursive compact key evidence helper direct hash-shape gate",
    ],
    [
      "--negative-control-compact-key-helper-direct-hash-read-failure",
      /\{label\} could not be read[\s\S]*?except OSError:[\s\S]*?raise/u,
      "ABI-7 recursive compact key evidence helper direct hash read-failure gate",
    ],
    [
      "--negative-control-compact-key-helper-generator-log-strict-read",
      /UnicodeDecodeError:[\s\S]*?\{label\} could not be read[\s\S]*?UnicodeDecodeError:[\s\S]*?raise/u,
      "ABI-7 recursive compact key evidence helper generator-log strict-read gate",
    ],
    [
      "--negative-control-compact-key-helper-artifact-open-path-binding",
      /expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "ABI-7 recursive compact key evidence helper artifact open path binding",
    ],
    [
      "--negative-control-compact-key-helper-future-skew",
      /_validate_generated_at_future_skew\([\s\S]*?max_generated_at_future_skew_seconds[\s\S]*?_skip_generated_at_future_skew\(/u,
      "ABI-7 recursive compact key evidence helper future-skew gate",
    ],
    [
      "--negative-control-compact-key-helper-output-early-preflight",
      /path_errors\.extend\(preflight_output_path\(out_path, "--out"\)\)[\s\S]*?path_errors\.extend\(\[\]\)/u,
      "ABI-7 recursive compact key evidence helper output early preflight gate",
    ],
    [
      "--negative-control-compact-key-helper-output-parent-create-failure",
      /if not parent_exists:[\s\S]*?parent\.mkdir\(mode=0o700, parents=True, exist_ok=True\)[\s\S]*?\{label\} parent directory could not be created[\s\S]*?parent\.mkdir\(mode=0o700, parents=True, exist_ok=True\)/u,
      "ABI-7 recursive compact key evidence helper output parent-create failure gate",
    ],
    [
      "--negative-control-compact-key-helper-output-file-metadata-failure",
      /output_mode = path\.lstat\(\)\.st_mode[\s\S]*?except OSError:[\s\S]*?\{label\} file metadata could not be read[\s\S]*?except FileNotFoundError:[\s\S]*?return \[\]/u,
      "ABI-7 recursive compact key evidence helper output file metadata failure gate",
    ],
    [
      "--negative-control-compact-key-helper-output-hardlink-metadata-failure",
      /link_count = path\.stat\(\)\.st_nlink[\s\S]*?except OSError:[\s\S]*?\{label\} hardlink metadata could not be read[\s\S]*?link_count = path\.stat\(\)\.st_nlink/u,
      "ABI-7 recursive compact key evidence helper output hardlink metadata failure gate",
    ],
    [
      "--negative-control-compact-key-helper-output-write-failure",
      /os\.replace\([\s\S]*?src_dir_fd=parent_fd[\s\S]*?dst_dir_fd=parent_fd[\s\S]*?path\.write_text\(evidence_text, encoding="utf-8"\)/u,
      "ABI-7 recursive compact key evidence helper output write-failure gate",
    ],
    [
      "--negative-control-compact-key-helper-output-temp-cleanup-failure",
      /return \["--out temporary file could not be removed"\][\s\S]*?return \[\]/u,
      "ABI-7 recursive compact key evidence helper output temp cleanup-failure gate",
    ],
    [
      "--negative-control-compact-key-helper-output-temp-cleanup-identity",
      /_file_identity\(temp_stat\) != expected_identity[\s\S]*?False/u,
      "ABI-7 recursive compact key evidence helper output temp cleanup identity",
    ],
    [
      "--negative-control-compact-key-helper-output-published-cleanup-identity",
      /_file_identity\(file_stat\) != expected_identity[\s\S]*?False/u,
      "ABI-7 recursive compact key evidence helper output published cleanup identity",
    ],
    [
      "--negative-control-compact-key-helper-strict-json-write",
      /allow_nan=False[\s\S]*?\["--out evidence is not strict JSON"\][\s\S]*?allow_nan=True/u,
      "ABI-7 recursive compact key evidence helper strict JSON writer",
    ],
    [
      "--negative-control-compact-key-helper-output-readback-verification",
      /readback_text != evidence_text[\s\S]*?False/u,
      "ABI-7 recursive compact key evidence helper output readback verification gate",
    ],
    [
      "--negative-control-compact-key-helper-output-readback-failure",
      /except OSError:[\s\S]*?return None, \[f"\{label\} write verification failed"\][\s\S]*?except OSError:[\s\S]*?return None, \[\]/u,
      "ABI-7 recursive compact key evidence helper output readback failure gate",
    ],
    [
      "--negative-control-compact-key-helper-output-readback-open-path-binding",
      /output_expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?output_expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "ABI-7 recursive compact key evidence helper output readback open-path binding",
    ],
    [
      "--negative-control-compact-key-helper-output-parent-sync-identity",
      /expected_identity=parent_identity[\s\S]*?expected_identity=None/u,
      "ABI-7 recursive compact key evidence helper output parent sync identity",
    ],
    [
      "--negative-control-compact-key-helper-output-post-write-preflight",
      /sync_errors = _sync_output_parent_fd\([\s\S]*?errors = validate_output_path\(path, "--out"\)[\s\S]*?if errors:[\s\S]*?return errors[\s\S]*?sync_errors = _sync_output_parent_fd\(/u,
      "ABI-7 recursive compact key evidence helper output post-write preflight gate",
    ],
    [
      "--negative-control-compact-key-finalizer-exit-marker",
      /staged keygen exit code must be 0[\s\S]*?staged keygen exit code is advisory/u,
      "ABI-7 recursive compact key staged finalizer exit-marker gate",
    ],
    [
      "--negative-control-compact-key-finalizer-timestamp-raw",
      /compact_evidence\._validate_generated_at_utc\(args\.generated_at_utc\)[\s\S]*?\[\]/u,
      "ABI-7 recursive compact key staged finalizer raw timestamp gate",
    ],
    [
      "--negative-control-compact-key-finalizer-future-skew",
      /compact_evidence\._validate_generated_at_future_skew\([\s\S]*?generated_at,[\s\S]*?args\.max_generated_at_future_skew_seconds[\s\S]*?compact_evidence\._skip_generated_at_future_skew\(/u,
      "ABI-7 recursive compact key staged finalizer future-skew preflight",
    ],
    [
      "--negative-control-compact-key-finalizer-publish-readback",
      /verify_errors = _verify_published_file_at\([\s\S]*?verify_errors = _trust_published_file_at\(/u,
      "ABI-7 recursive compact key staged finalizer publish readback",
    ],
    [
      "--negative-control-compact-key-finalizer-publish-rollback-identity",
      /_file_identity\(path_stat\) == expected_identity[\s\S]*?True/u,
      "ABI-7 recursive compact key staged finalizer publish rollback identity",
    ],
    [
      "--negative-control-compact-key-finalizer-publish-rollback-cleanup-report",
      /return \[f"\{label\} rollback cleanup could not remove file"\][\s\S]*?return \[\]/u,
      "ABI-7 recursive compact key staged finalizer publish rollback cleanup report",
    ],
    [
      "--negative-control-compact-key-finalizer-publish-dir-sync-identity",
      /expected_identity=artifact_dir_identity[\s\S]*?expected_identity=None/u,
      "ABI-7 recursive compact key staged finalizer publish directory sync identity",
    ],
    [
      "--negative-control-compact-key-finalizer-temp-cleanup-identity",
      /_file_identity\(temp_parent_stat\) != expected_identity[\s\S]*?False/u,
      "ABI-7 recursive compact staged finalizer temporary cleanup identity",
    ],
    [
      "--negative-control-compact-key-finalizer-temp-cleanup-report",
      /if finalizer_errors or cleanup_errors:[\s\S]*?if finalizer_errors:/u,
      "ABI-7 recursive compact staged finalizer temporary cleanup report",
    ],
    [
      "--negative-control-compact-key-staged-runner-exit-marker",
      /f"\{exit_code\}\\\\n"[\s\S]*?"0\\\\n"/u,
      "ABI-7 recursive compact key staged runner exit-marker preservation",
    ],
    [
      "--negative-control-compact-key-staged-runner-readback",
      /return _verify_written_text_file\(path, expected_bytes, label\)[\s\S]*?return \[\]/u,
      "ABI-7 recursive compact key staged runner metadata readback",
    ],
    [
      "--negative-control-compact-key-staged-runner-parent-sync-identity",
      /expected_identity=parent_identity[\s\S]*?expected_identity=None[\s\S]*?_file_identity\(current_parent_stat\) != parent_identity[\s\S]*?False/u,
      "ABI-7 recursive compact key staged runner parent sync identity",
    ],
    [
      "--negative-control-compact-key-staged-runner-log-install-parent-sync-identity",
      /expected_identity=log_parent_identity[\s\S]*?expected_identity=None[\s\S]*?_file_identity\(current_log_parent_stat\) != log_parent_identity[\s\S]*?False/u,
      "ABI-7 recursive compact key staged runner log-install parent sync identity",
    ],
    [
      "--negative-control-compact-key-staged-runner-cleanup-identity",
      /_file_identity\(path_stat\) != expected_identity[\s\S]*?False/u,
      "ABI-7 recursive compact key staged runner cleanup identity",
    ],
    [
      "--negative-control-compact-key-staged-runner-published-cleanup-report",
      /cleanup_errors = _unlink_file_if_identity_at\([\s\S]*?rollback_blockers = _unlink_file_if_identity_at\(/u,
      "ABI-7 recursive compact key staged runner published cleanup report",
    ],
    [
      "--negative-control-compact-key-staged-runner-child-log-file",
      /stdout=log_handle[\s\S]*?stdout=subprocess\.PIPE/u,
      "ABI-7 recursive compact key staged runner child log-file binding",
    ],
    [
      "--negative-control-compact-key-staged-runner-supervisor-output-pipe",
      /break\\n            except subprocess\.TimeoutExpired:[\s\S]*?sys\.stdout\.buffer\.write\(b\\"\\"\)[\s\S]*?break\\n            except subprocess\.TimeoutExpired:/u,
      "ABI-7 recursive compact key staged runner supervisor output pipe",
    ],
    [
      "--negative-control-compact-key-staged-runner-execution-log-sha256",
      /generator_log_sha256 must match staged generator log SHA-256[\s\S]*?generator_log_sha256 may drift from staged generator log SHA-256/u,
      "ABI-7 recursive compact key staged runner execution-log SHA-256 binding",
    ],
    [
      "--negative-control-compact-key-staged-runner-resume-replace-conflict",
      /--replace and --resume-keygen cannot be combined[\s\S]*?--replace and --resume-keygen may be combined/u,
      "ABI-7 recursive compact key staged runner resume/replace conflict gate",
    ],
    [
      "--negative-control-doc-route",
      /roadmap\.md[\s\S]*?Reserved-lineage recursive spend path[\s\S]*?semantic aggregation compact path/u,
      "production route docs",
    ],
    [
      "--negative-control-evidence-helper-path-aliases",
      /evidence_helper_alias_checks[\s\S]*?must not contain backslashes[\s\S]*?must be canonical[\s\S]*?kagemusha_lineage_proof_evidence\.py[\s\S]*?kagemusha_recursive_compact_key_evidence\.py/u,
      "Kagemusha evidence helper path alias gate",
    ],
    [
      "--negative-control-json-duplicate-keys",
      /object_pairs_hook=_reject_duplicate_json_object_pairs[\s\S]*?object_pairs_hook=dict/u,
      "Kagemusha readiness duplicate JSON key gate",
    ],
    [
      "--negative-control-kagemusha-readiness-json-read-failure",
      /except OSError:[\s\S]*?blocker\(unreadable_code, f"\{label\} could not be read"\)[\s\S]*?elif error == unreadable_error/u,
      "Kagemusha readiness JSON read/decode failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-json-open-path-binding",
      /digest, text, read_errors = _sha256_text_file\([\s\S]*?_sha256_text_file_unbound\(/u,
      "Kagemusha readiness JSON open-path binding",
    ],
    [
      "--negative-control-kagemusha-readiness-release-json-direct-secret-paths",
      /def _validate_release_local_json_file_for_read[\s\S]*?SECRET_RE\.search\(path_text\)[\s\S]*?\{label\} path must not contain secret-looking material[\s\S]*?def _validate_release_local_json_file_for_read/u,
      "Kagemusha readiness release JSON direct secret-path gate",
    ],
    [
      "--negative-control-kagemusha-readiness-release-json-direct-path-aliases",
      /path must not contain backslashes[\s\S]*?path must be canonical[\s\S]*?release_json_ancestor_errors = device_lab\.validate_no_symlink_ancestors/u,
      "Kagemusha readiness release JSON direct path-alias gate",
    ],
    [
      "--negative-control-kagemusha-readiness-release-json-hardlink-metadata-failure",
      /link_count = path\.stat\(\)\.st_nlink[\s\S]*?\{label\} hardlink metadata could not be read[\s\S]*?link_count = path\.stat\(\)\.st_nlink/u,
      "Kagemusha readiness release JSON hardlink metadata failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-release-json-file-metadata-failure",
      /file_stat = path\.lstat\(\)[\s\S]*?\{label\} file metadata could not be read[\s\S]*?file_stat = path\.lstat\(\)/u,
      "Kagemusha readiness release JSON file metadata failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-release-json-size-limit",
      /if open_stat\.st_size > max_bytes:[\s\S]*?if False and open_stat\.st_size > max_bytes:/u,
      "Kagemusha readiness release JSON size limit",
    ],
    [
      "--negative-control-kagemusha-readiness-release-json-open-path-binding",
      /release_json_expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?release_json_expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Kagemusha readiness release JSON open-path binding",
    ],
    [
      "--negative-control-kagemusha-readiness-repo-root-aliases",
      /repo_root_errors = validate_repo_root_path\(Path\(args\.repo_root\)\)[\s\S]*?repo_root_errors = \[\]/u,
      "Kagemusha readiness repo-root alias gate",
    ],
    [
      "--negative-control-kagemusha-readiness-repo-root-direct-secret-paths",
      /label="--repo-root"[\s\S]*?code="kagemusha_repo_root_path_invalid"[\s\S]*?return \[secret_blocker\][\s\S]*?""/u,
      "Kagemusha readiness direct repo-root secret-path gate",
    ],
    [
      "--negative-control-kagemusha-readiness-repo-root-metadata-failure",
      /root\.lstat\(\)\.st_mode[\s\S]*?--repo-root metadata could not be read[\s\S]*?root\.lstat\(\)\.st_mode[\s\S]*?root_mode = None/u,
      "Kagemusha readiness direct repo-root metadata failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-repo-root-resolve-failure",
      /Path\(args\.repo_root\)\.resolve\(\)[\s\S]*?--repo-root could not be resolved[\s\S]*?repo_root = Path\(args\.repo_root\)\.resolve\(\)/u,
      "Kagemusha readiness repo-root resolve-failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-rollup",
      /android_device_lab_standard_matrix_missing[\s\S]*?android_device_lab_matrix_optional/u,
      "Kagemusha production readiness evidence rollup",
    ],
    [
      "--negative-control-kagemusha-readiness-rollup-path-safety",
      /path_blockers = validate_cli_path_arguments\(args\)[\s\S]*?path_blockers = \[\]/u,
      "Kagemusha readiness rollup path safety",
    ],
    [
      "--negative-control-kagemusha-readiness-source-marker-direct-secret-paths",
      /def _validate_repo_source_marker_file_for_read[\s\S]*?SECRET_RE\.search\(path_text\)[\s\S]*?\{label\} path must not contain secret-looking material[\s\S]*?def _validate_repo_source_marker_file_for_read/u,
      "Kagemusha readiness source marker direct secret-path gate",
    ],
    [
      "--negative-control-kagemusha-readiness-source-marker-direct-path-aliases",
      /path must not contain backslashes[\s\S]*?path must be canonical[\s\S]*?errors = \[/u,
      "Kagemusha readiness source marker direct path-alias gate",
    ],
    [
      "--negative-control-kagemusha-readiness-source-marker-hardlink-metadata-failure",
      /link_count = path\.stat\(\)\.st_nlink[\s\S]*?\{label\} hardlink metadata could not be read[\s\S]*?def validate_repo_source_marker_file/u,
      "Kagemusha readiness source marker hardlink metadata failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-source-marker-file-metadata-failure",
      /file_stat = path\.lstat\(\)[\s\S]*?\{label\} file metadata could not be read[\s\S]*?stat\.S_ISLNK\(file_stat\.st_mode\)/u,
      "Kagemusha readiness source marker file metadata failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-source-marker-read-preflight",
      /_repo_source_marker_text\([\s\S]*?path\.read_text\(encoding="utf-8"\)/u,
      "Kagemusha readiness source marker read preflight gate",
    ],
    [
      "--negative-control-kagemusha-readiness-source-marker-open-path-binding",
      /expected_marker_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?expected_marker_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Kagemusha readiness source marker open-path binding",
    ],
    [
      "--negative-control-kagemusha-readiness-source-marker-non-utf8-read",
      /except UnicodeDecodeError:[\s\S]*?return None, \[unreadable_error\][\s\S]*?return "", \[\]/u,
      "Kagemusha readiness source marker non-UTF-8 read gate",
    ],
    [
      "--negative-control-kagemusha-readiness-source-marker-size-limit",
      /if open_stat\.st_size > MAX_REPO_SOURCE_MARKER_BYTES:[\s\S]*?if False and open_stat\.st_size > MAX_REPO_SOURCE_MARKER_BYTES:/u,
      "Kagemusha readiness source marker size-limit gate",
    ],
    [
      "--negative-control-kagemusha-readiness-trusted-signer-sanitization",
      /device_lab\._trusted_signer_public_key_sha256_set\([\s\S]*?set\(/u,
      "Kagemusha readiness trusted-signer summary sanitization",
    ],
    [
      "--negative-control-kagemusha-readiness-android-report-secret-redaction",
      /android_device_lab_report_unsafe_material[\s\S]*?android_device_lab_report_redaction_disabled/u,
      "Kagemusha readiness Android report unsafe-string redaction",
    ],
    [
      "--negative-control-kagemusha-readiness-android-zero-binding-digest",
      /or value == "0" \* 64[\s\S]*?or False/u,
      "Kagemusha readiness Android zero binding digest",
    ],
    [
      "--negative-control-kagemusha-readiness-trust-root-section-preflight",
      /repo_root_blockers = validate_repo_root_path\(repo_root\)[\s\S]*?details[\s\S]*?repo_root_blockers = validate_repo_root_path\(repo_root\)[\s\S]*?circuit_id[\s\S]*?repo_root_blockers = validate_repo_root_path\(repo_root\)[\s\S]*?checked_files/u,
      "Kagemusha readiness trust-root section repo-root preflight",
    ],
    [
      "--negative-control-kagemusha-readiness-android-root-discovery-read-failure",
      /android_device_lab_root_unreadable[\s\S]*?android_device_lab_root_listing_failures_ignored/u,
      "Kagemusha readiness Android root discovery read-failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-aliases",
      /--summary-out must not be a symlink[\s\S]*?--summary-out may be a symlink/u,
      "Kagemusha readiness summary output alias gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-dangling-alias",
      /if stat\.S_ISLNK\(summary_output_mode\):[\s\S]*?if stat\.S_ISLNK\(summary_output_mode\) and path\.exists\(\):/u,
      "Kagemusha readiness summary output dangling alias gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-ancestor",
      /validate_no_symlink_ancestors\([\s\S]*?--summary-out ancestor directory[\s\S]*?if not parent_exists:/u,
      "Kagemusha readiness summary output ancestor alias gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-parent-is-dir-preflight",
      /if not stat\.S_ISDIR\(parent_mode\):[\s\S]*?if not parent\.is_dir\(\):/u,
      "Kagemusha readiness summary output parent is_dir preflight gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-parent-metadata-failure",
      /--summary-out parent directory metadata could not be read[\s\S]*?return False, \[\]/u,
      "Kagemusha readiness summary output parent metadata failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-parent-create-failure",
      /parent\.mkdir\(parents=True, exist_ok=True\)[\s\S]*?--summary-out parent directory could not be created[\s\S]*?parent\.mkdir\(parents=True, exist_ok=True\)/u,
      "Kagemusha readiness summary output parent-create failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-post-create-parent-preflight",
      /parent_exists, parent_blockers = _validate_summary_output_parent\([\s\S]*?--summary-out parent must be a directory[\s\S]*?validate_no_symlink_ancestors\([\s\S]*?""/u,
      "Kagemusha readiness summary output post-create parent preflight gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-regular-file",
      /if not stat\.S_ISREG\(summary_output_mode\):[\s\S]*?if False and not stat\.S_ISREG\(summary_output_mode\):/u,
      "Kagemusha readiness summary output regular-file gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-file-metadata-failure",
      /summary_output_mode = path\.lstat\(\)\.st_mode[\s\S]*?--summary-out file metadata could not be read[\s\S]*?summary_output_mode = path\.lstat\(\)\.st_mode/u,
      "Kagemusha readiness summary output file metadata failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-hardlink-metadata-failure",
      /link_count = path\.stat\(\)\.st_nlink[\s\S]*?--summary-out hardlink metadata could not be read[\s\S]*?link_count = path\.stat\(\)\.st_nlink/u,
      "Kagemusha readiness summary output hardlink metadata failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-direct-secret-paths",
      /label="--summary-out"[\s\S]*?code=SUMMARY_OUT_PATH_INVALID_CODE[\s\S]*?return \[secret_blocker\][\s\S]*?""/u,
      "Kagemusha readiness direct summary output secret-path gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-write-failure",
      /os\.replace\([\s\S]*?src_dir_fd=parent_fd[\s\S]*?dst_dir_fd=parent_fd[\s\S]*?path\.write_text\(summary_text, encoding="utf-8"\)/u,
      "Kagemusha readiness summary output write-failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-temp-cleanup-failure",
      /"--summary-out temporary file could not be removed"[\s\S]*?"--summary-out temp cleanup is optional"/u,
      "Kagemusha readiness summary output temp cleanup-failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-temp-cleanup-identity",
      /_file_identity\(temp_stat\) != expected_identity[\s\S]*?False/u,
      "Kagemusha readiness summary output temp cleanup identity",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-published-cleanup-identity",
      /_file_identity\(file_stat\) != expected_identity[\s\S]*?False/u,
      "Kagemusha readiness summary output published cleanup identity",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-strict-json-write",
      /allow_nan=False[\s\S]*?allow_nan=True/u,
      "Kagemusha readiness summary output strict JSON writer",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-size-limit",
      /if len\(summary_text\.encode\("utf-8"\)\) > MAX_READINESS_SUMMARY_JSON_BYTES:[\s\S]*?if False and len\(summary_text\.encode\("utf-8"\)\) > MAX_READINESS_SUMMARY_JSON_BYTES:/u,
      "Kagemusha readiness summary output size-limit gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-readback-verification",
      /readback_text != summary_text[\s\S]*?False/u,
      "Kagemusha readiness summary output readback gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-readback-failure",
      /except OSError:[\s\S]*?_summary_out_blocker\("--summary-out write verification failed"\)[\s\S]*?except OSError:[\s\S]*?return None, \[\]/u,
      "Kagemusha readiness summary output readback failure gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-readback-size-limit",
      /if open_stat\.st_size > MAX_READINESS_SUMMARY_JSON_BYTES:[\s\S]*?if False and open_stat\.st_size > MAX_READINESS_SUMMARY_JSON_BYTES:/u,
      "Kagemusha readiness summary output readback size-limit gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-readback-open-path-binding",
      /summary_expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?summary_expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Kagemusha readiness summary output readback open-path binding gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-parent-sync-identity",
      /expected_identity=parent_identity[\s\S]*?expected_identity=None/u,
      "Kagemusha readiness summary output parent sync identity gate",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-post-write-preflight",
      /errors = validate_summary_output_path\(path\)[\s\S]*?if errors:[\s\S]*?return errors[\s\S]*?if stat\.S_ISLNK\(expected_stat\.st_mode\):/u,
      "Kagemusha readiness summary output post-write preflight gate",
    ],
    [
      "--negative-control-lineage-key-release-tooling",
      /record_out: Option<std::path::PathBuf>[\s\S]*?record_archive_out: Option<std::path::PathBuf>/u,
      "Reserved-lineage key release tooling",
    ],
    [
      "--negative-control-lineage-proof-evidence",
      /lineage_proof_evidence_missing[\s\S]*?lineage_proof_evidence_optional/u,
      "Reserved-lineage production proof evidence",
    ],
    [
      "--negative-control-lineage-proof-evidence-path-aliases",
      /lineage_proof_evidence_path=lineage_proof_evidence_path,[\s\S]*?lineage_proof_evidence_path=lineage_proof_evidence_path\.resolve\(\),/u,
      "Reserved-lineage proof evidence path alias gate",
    ],
    [
      "--negative-control-lineage-proof-local-secret-paths",
      /SECRET_RE\.search\(path_text\)[\s\S]*?\{label\} path must not contain secret-looking material[\s\S]*?""/u,
      "Reserved-lineage proof evidence local secret-path gate",
    ],
    [
      "--negative-control-lineage-proof-local-path-aliases",
      /path must not contain backslashes[\s\S]*?path must be canonical[\s\S]*?ancestor_errors = device_lab\.validate_no_symlink_ancestors/u,
      "Reserved-lineage proof evidence local path-alias gate",
    ],
    [
      "--negative-control-lineage-proof-local-ancestor-aliases",
      /ancestor_errors = device_lab\.validate_no_symlink_ancestors\([\s\S]*?\{label\} ancestor directory[\s\S]*?return None, ancestor_errors[\s\S]*?""/u,
      "Reserved-lineage proof evidence local ancestor alias gate",
    ],
    [
      "--negative-control-lineage-proof-local-hardlink-metadata-failure",
      /link_count = path\.stat\(\)\.st_nlink[\s\S]*?\{label\} hardlink metadata could not be read[\s\S]*?link_count = path\.stat\(\)\.st_nlink/u,
      "Reserved-lineage proof evidence local hardlink metadata failure gate",
    ],
    [
      "--negative-control-lineage-proof-local-file-metadata-failure",
      /file_stat = path\.lstat\(\)[\s\S]*?\{label\} file metadata could not be read[\s\S]*?\{label\} is missing/u,
      "Reserved-lineage proof evidence local file metadata failure gate",
    ],
    [
      "--negative-control-lineage-proof-artifact-binding",
      /lineage_proof_evidence_artifact_file_digest[\s\S]*?lineage_proof_evidence_artifact_self_report_only/u,
      "Reserved-lineage proof evidence artifact byte binding",
    ],
    [
      "--negative-control-lineage-proof-artifact-is-file-preflight",
      /_sha256_file_with_size_and_prefix\([\s\S]*?artifact_path\.is_file\(\)/u,
      "Reserved-lineage proof evidence artifact is_file preflight gate",
    ],
    [
      "--negative-control-lineage-proof-file-aliases",
      /lineage_proof_evidence_artifact_file_shape[\s\S]*?lineage_proof_evidence_artifact_file_alias_allowed/u,
      "Reserved-lineage proof evidence file alias gate",
    ],
    [
      "--negative-control-lineage-proof-future-skew",
      /lineage_proof_evidence_future_dated[\s\S]*?lineage_proof_evidence_allows_future_dated/u,
      "Reserved-lineage proof evidence future-skew gate",
    ],
    [
      "--negative-control-lineage-proof-artifact-prefix-binding",
      /validate_lineage_artifact_prefix\(artifact_prefix, artifact\)[\s\S]*?validate_lineage_artifact_content\(artifact_path, artifact\)[\s\S]*?readiness\.validate_lineage_artifact_prefix\(artifact_prefix, artifact\)[\s\S]*?readiness\.validate_lineage_artifact_content\(path, artifact\)/u,
      "Reserved-lineage proof evidence artifact prefix binding",
    ],
    [
      "--negative-control-lineage-proof-command-canonical",
      /must exactly match the canonical production Reserved-lineage proof command string[\s\S]*?canonical command spelling accepted/u,
      "Reserved-lineage proof evidence canonical command gate",
    ],
    [
      "--negative-control-lineage-proof-scalar-types",
      /not isinstance\(scalar_value, int\)[\s\S]*?False/u,
      "Reserved-lineage proof evidence scalar type gate",
    ],
    [
      "--negative-control-lineage-proof-artifact-size-binding",
      /_require_lineage_artifact_size[\s\S]*?_lineage_artifact_size_disabled/u,
      "Reserved-lineage proof evidence artifact size binding",
    ],
    [
      "--negative-control-lineage-proof-evidence-json-size-limit",
      /max_bytes=MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES[\s\S]*?max_bytes=None/u,
      "Reserved-lineage proof evidence JSON size limit",
    ],
    [
      "--negative-control-lineage-proof-readiness-artifact-open-path-binding",
      /expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Reserved-lineage proof readiness artifact open-path binding",
    ],
    [
      "--negative-control-lineage-proof-helper-timestamp-raw",
      /errors\.extend\(_validate_generated_at_utc\(generated_at_utc\)\)[\s\S]*?errors\.extend\(\[\]\)/u,
      "Reserved-lineage proof evidence helper raw timestamp gate",
    ],
    [
      "--negative-control-lineage-proof-helper-future-skew",
      /_validate_generated_at_future_skew\([\s\S]*?max_generated_at_future_skew_seconds[\s\S]*?_skip_generated_at_future_skew\(/u,
      "Reserved-lineage proof evidence helper future-skew gate",
    ],
    [
      "--negative-control-lineage-proof-helper-strict-json-write",
      /allow_nan=False[\s\S]*?\["--out evidence is not strict JSON"\][\s\S]*?allow_nan=True/u,
      "Reserved-lineage proof evidence helper strict JSON writer",
    ],
    [
      "--negative-control-lineage-proof-helper-artifact-open-path-binding",
      /expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Reserved-lineage proof evidence helper artifact open path binding",
    ],
    [
      "--negative-control-lineage-proof-helper-direct-secret-paths",
      /secret_error = _secret_path_error\(str\(artifact_dir\), "--artifact-dir"\)[\s\S]*?secret_error = None[\s\S]*?secret_error = _secret_path_error\(str\(path\), label\)[\s\S]*?secret_error = None/u,
      "Reserved-lineage proof evidence helper direct secret-path gates",
    ],
    [
      "--negative-control-lineage-proof-helper-direct-hash-shape",
      /readiness\._validate_lineage_local_file_for_read\([\s\S]*?return None, file_errors[\s\S]*?expected_stat = path\.stat\(\)/u,
      "Reserved-lineage proof evidence helper direct hash path-shape gate",
    ],
    [
      "--negative-control-lineage-proof-helper-direct-hash-read-failure",
      /\{label\} could not be read[\s\S]*?except OSError:[\s\S]*?raise/u,
      "Reserved-lineage proof evidence helper direct hash read-failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-direct-artifact-dir-secret-paths",
      /def validate_artifact_dir_path[\s\S]*?secret_error = _secret_path_error\(str\(artifact_dir\), "--artifact-dir"\)[\s\S]*?return \[secret_error\][\s\S]*?def validate_artifact_dir_path/u,
      "Reserved-lineage proof evidence helper direct artifact-dir secret-path gate",
    ],
    [
      "--negative-control-lineage-proof-helper-direct-artifact-dir-metadata-failure",
      /artifact_dir\.lstat\(\)\.st_mode[\s\S]*?--artifact-dir metadata could not be read[\s\S]*?artifact_dir\.lstat\(\)\.st_mode/u,
      "Reserved-lineage proof evidence helper direct artifact-dir metadata failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-direct-proof-log-secret-paths",
      /proof_log_secret_error = _secret_path_error\(str\(proof_log\), "--proof-log"\)[\s\S]*?return \[proof_log_secret_error\][\s\S]*?""/u,
      "Reserved-lineage proof evidence helper direct proof-log secret-path gate",
    ],
    [
      "--negative-control-lineage-proof-helper-direct-output-preflight-secret-paths",
      /def preflight_output_path[\s\S]*?secret_error = _secret_path_error\(str\(path\), label\)[\s\S]*?return \[secret_error\][\s\S]*?def preflight_output_path/u,
      "Reserved-lineage proof evidence helper direct output-preflight secret-path gate",
    ],
    [
      "--negative-control-lineage-proof-helper-validation-dir-aliases",
      /pre_create_dir_errors = validate_artifact_dir_path\(artifact_dir\)[\s\S]*?pre_create_dir_errors = \[\]/u,
      "Reserved-lineage proof evidence helper validation dir alias gate",
    ],
    [
      "--negative-control-lineage-proof-helper-validation-dir-create-failure",
      /artifact_dir\.mkdir\(mode=0o700, parents=True, exist_ok=True\)[\s\S]*?--artifact-dir could not be created for evidence validation[\s\S]*?artifact_dir\.mkdir\(mode=0o700, parents=True, exist_ok=True\)/u,
      "Reserved-lineage proof evidence helper validation dir create-failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-validation-strict-json-write",
      /lineage proof evidence validation file is not strict JSON[\s\S]*?lineage proof evidence validation file allows non-strict JSON/u,
      "Reserved-lineage proof evidence helper validation strict JSON gate",
    ],
    [
      "--negative-control-lineage-proof-helper-validation-temp-write-failure",
      /lineage proof evidence validation file could not be written[\s\S]*?lineage proof evidence validation file write failures ignored/u,
      "Reserved-lineage proof evidence helper validation temp write-failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-validation-temp-cleanup-after-write-failure",
      /errors\.extend\(_cleanup_validation_temp_output\(path, tmp_identity\)\)[\s\S]*?pass/u,
      "Reserved-lineage proof evidence helper validation temp cleanup after write-failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-validation-temp-cleanup-failure",
      /lineage proof evidence validation file could not be removed[\s\S]*?lineage proof evidence validation file cleanup failures ignored/u,
      "Reserved-lineage proof evidence helper validation temp cleanup-failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-validation-temp-cleanup-identity",
      /_file_identity\(validation_temp_stat\) != expected_identity[\s\S]*?False/u,
      "Reserved-lineage proof evidence helper validation temp cleanup identity",
    ],
    [
      "--negative-control-lineage-proof-helper-input-corridor",
      /errors = validate_lineage_input_paths\(artifact_dir, proof_log\)[\s\S]*?errors = \[\][\s\S]*?path_errors\.extend\(validate_lineage_input_paths\(artifact_dir, proof_log\)\)[\s\S]*?path_errors\.extend\(\[\]\)/u,
      "Reserved-lineage proof evidence helper input corridor",
    ],
    [
      "--negative-control-lineage-proof-helper-input-corridor-resolve-failure",
      /same_parent, corridor_errors = _same_resolved_parent\(proof_log, artifact_dir\)[\s\S]*?same_parent = proof_log\.parent\.resolve\(\) == artifact_dir\.resolve\(\)/u,
      "Reserved-lineage proof evidence helper input corridor resolve-failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-aliases",
      /--artifact-dir must not be a symlink[\s\S]*?--artifact-dir may be a symlink/u,
      "Reserved-lineage proof evidence helper output alias gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-dangling-alias",
      /if stat\.S_ISLNK\(output_mode\):[\s\S]*?if stat\.S_ISLNK\(output_mode\) and path\.exists\(\):/u,
      "Reserved-lineage proof evidence helper dangling output alias gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-ancestor",
      /output_ancestor_errors = device_lab\.validate_no_symlink_ancestors\([\s\S]*?return output_ancestor_errors[\s\S]*?if not parent_exists:/u,
      "Reserved-lineage proof evidence helper output ancestor gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-parent-is-dir-preflight",
      /if not stat\.S_ISDIR\(parent_mode\):[\s\S]*?if not parent\.is_dir\(\):/u,
      "Reserved-lineage proof evidence helper output parent is_dir preflight gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-parent-metadata-failure",
      /parent directory metadata could not be read[\s\S]*?except OSError:[\s\S]*?return False, \[\]/u,
      "Reserved-lineage proof evidence helper output parent metadata failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-parent-create-failure",
      /if not parent_exists:[\s\S]*?parent\.mkdir\(mode=0o700, parents=True, exist_ok=True\)[\s\S]*?\{label\} parent directory could not be created[\s\S]*?parent\.mkdir\(mode=0o700, parents=True, exist_ok=True\)/u,
      "Reserved-lineage proof evidence helper output parent-create failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-post-create-parent-preflight",
      /missing_error=f"\{label\} parent must be a directory"[\s\S]*?return \[f"\{label\} parent must be a directory"\][\s\S]*?""/u,
      "Reserved-lineage proof evidence helper output post-create parent preflight gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-validate-parent-create-failure",
      /errors = preflight_output_path\(path, label\)[\s\S]*?parent\.mkdir\(mode=0o700, parents=True, exist_ok=True\)[\s\S]*?\{label\} parent directory could not be created[\s\S]*?parent\.mkdir\(mode=0o700, parents=True, exist_ok=True\)/u,
      "Reserved-lineage proof evidence helper output validator parent-create failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-file-metadata-failure",
      /output_mode = path\.lstat\(\)\.st_mode[\s\S]*?except OSError:[\s\S]*?\{label\} file metadata could not be read[\s\S]*?except FileNotFoundError:[\s\S]*?return \[\]/u,
      "Reserved-lineage proof evidence helper output file metadata failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-hardlink-metadata-failure",
      /link_count = path\.stat\(\)\.st_nlink[\s\S]*?except OSError:[\s\S]*?\{label\} hardlink metadata could not be read[\s\S]*?link_count = path\.stat\(\)\.st_nlink/u,
      "Reserved-lineage proof evidence helper output hardlink metadata failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-early-preflight",
      /early_output_errors = preflight_output_path\(out_path, "--out"\)[\s\S]*?early_output_errors = \[\]/u,
      "Reserved-lineage proof evidence helper early output preflight gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-write-failure",
      /os\.replace\([\s\S]*?src_dir_fd=parent_fd[\s\S]*?dst_dir_fd=parent_fd[\s\S]*?path\.write_text\(evidence_text, encoding="utf-8"\)/u,
      "Reserved-lineage proof evidence helper output write-failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-temp-cleanup-failure",
      /return \["--out temporary file could not be removed"\][\s\S]*?return \[\]/u,
      "Reserved-lineage proof evidence helper output temp cleanup-failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-temp-cleanup-identity",
      /_file_identity\(temp_stat\) != expected_identity[\s\S]*?False/u,
      "Reserved-lineage proof evidence helper output temp cleanup identity",
    ],
    [
      "--negative-control-lineage-proof-helper-output-published-cleanup-identity",
      /_file_identity\(file_stat\) != expected_identity[\s\S]*?False/u,
      "Reserved-lineage proof evidence helper output published cleanup identity",
    ],
    [
      "--negative-control-lineage-proof-helper-output-readback-verification",
      /readback_text != evidence_text[\s\S]*?False/u,
      "Reserved-lineage proof evidence helper output readback verification gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-readback-failure",
      /except OSError:[\s\S]*?return None, \[f"\{label\} write verification failed"\][\s\S]*?except OSError:[\s\S]*?return None, \[\]/u,
      "Reserved-lineage proof evidence helper output readback failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-readback-open-path-binding",
      /output_expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?output_expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Reserved-lineage proof evidence helper output readback open-path binding",
    ],
    [
      "--negative-control-lineage-proof-helper-output-post-write-preflight",
      /sync_errors = _sync_output_parent_fd\([\s\S]*?errors = validate_output_path\(path, "--out"\)[\s\S]*?if errors:[\s\S]*?return errors[\s\S]*?sync_errors = _sync_output_parent_fd\(/u,
      "Reserved-lineage proof evidence helper output post-write preflight gate",
    ],
    [
      "--negative-control-lineage-proof-helper-output-corridor-resolve-failure",
      /path_errors\.extend\(validate_output_corridor\(out_path, artifact_dir\)\)[\s\S]*?out_path\.resolve\(\)\.parent != artifact_dir\.resolve\(\)/u,
      "Reserved-lineage proof evidence helper output corridor resolve-failure gate",
    ],
    [
      "--negative-control-lineage-proof-finalizer-exit-marker",
      /staged lineage proof exit code must be 0[\s\S]*?staged lineage proof exit code is advisory/u,
      "Reserved-lineage proof staged finalizer exit-marker gate",
    ],
    [
      "--negative-control-lineage-proof-finalizer-timestamp-raw",
      /lineage_evidence\._validate_generated_at_utc\(args\.generated_at_utc\)[\s\S]*?\[\]/u,
      "Reserved-lineage proof staged finalizer raw timestamp gate",
    ],
    [
      "--negative-control-lineage-proof-finalizer-future-skew",
      /lineage_evidence\._validate_generated_at_future_skew\([\s\S]*?generated_at,[\s\S]*?args\.max_generated_at_future_skew_seconds[\s\S]*?lineage_evidence\._skip_generated_at_future_skew\(/u,
      "Reserved-lineage proof staged finalizer future-skew preflight",
    ],
    [
      "--negative-control-lineage-proof-finalizer-publish-readback",
      /verify_errors = _verify_published_file_at\([\s\S]*?verify_errors = _trust_published_file_at\(/u,
      "Reserved-lineage proof staged finalizer publish readback",
    ],
    [
      "--negative-control-lineage-proof-finalizer-publish-rollback-identity",
      /_file_identity\(path_stat\) == expected_identity[\s\S]*?True/u,
      "Reserved-lineage proof staged finalizer publish rollback identity",
    ],
    [
      "--negative-control-lineage-proof-finalizer-publish-rollback-cleanup-report",
      /return \[f"\{label\} rollback cleanup could not remove file"\][\s\S]*?return \[\]/u,
      "Reserved-lineage proof staged finalizer publish rollback cleanup report",
    ],
    [
      "--negative-control-lineage-proof-finalizer-publish-dir-sync-identity",
      /expected_identity=artifact_dir_identity[\s\S]*?expected_identity=None/u,
      "Reserved-lineage proof staged finalizer publish directory sync identity",
    ],
    [
      "--negative-control-lineage-proof-finalizer-temp-cleanup-identity",
      /_file_identity\(temp_parent_stat\) != expected_identity[\s\S]*?False/u,
      "Reserved-lineage proof staged finalizer temporary cleanup identity",
    ],
    [
      "--negative-control-lineage-proof-finalizer-temp-cleanup-report",
      /if finalizer_errors or cleanup_errors:[\s\S]*?if finalizer_errors:/u,
      "Reserved-lineage proof staged finalizer temporary cleanup report",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-exit-marker",
      /f"\{exit_code\}\\\\n"[\s\S]*?"0\\\\n"/u,
      "Reserved-lineage proof staged runner exit-marker preservation",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-readback",
      /return _verify_written_text_file\(path, expected_bytes, label\)[\s\S]*?return \[\]/u,
      "Reserved-lineage proof staged runner metadata readback",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-parent-sync-identity",
      /expected_identity=parent_identity[\s\S]*?expected_identity=None[\s\S]*?_file_identity\(current_parent_stat\) != parent_identity[\s\S]*?False/u,
      "Reserved-lineage proof staged runner parent sync identity",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-log-install-parent-sync-identity",
      /expected_identity=log_parent_identity[\s\S]*?expected_identity=None[\s\S]*?_file_identity\(current_log_parent_stat\) != log_parent_identity[\s\S]*?False/u,
      "Reserved-lineage proof staged runner log-install parent sync identity",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-cleanup-identity",
      /_file_identity\(path_stat\) != expected_identity[\s\S]*?False/u,
      "Reserved-lineage proof staged runner cleanup identity",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-published-cleanup-report",
      /cleanup_errors = _unlink_file_if_identity_at\([\s\S]*?rollback_blockers = _unlink_file_if_identity_at\(/u,
      "Reserved-lineage proof staged runner published cleanup report",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-child-log-file",
      /stdout=log_handle[\s\S]*?stdout=subprocess\.PIPE/u,
      "Reserved-lineage proof staged runner child log-file binding",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-supervisor-output-pipe",
      /break\\n            except subprocess\.TimeoutExpired:[\s\S]*?sys\.stdout\.buffer\.write\(b\\"\\"\)[\s\S]*?break\\n            except subprocess\.TimeoutExpired:/u,
      "Reserved-lineage proof staged runner supervisor output pipe",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-execution-log-sha256",
      /log_sha256 must match staged \{profile\} lineage key artifact log SHA-256[\s\S]*?log_sha256 may drift from staged \{profile\} lineage key artifact log/u,
      "Reserved-lineage proof staged runner execution-log SHA-256 binding",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-resume-replace-conflict",
      /--replace and --resume-key-artifacts cannot be combined[\s\S]*?--replace and --resume-key-artifacts may be combined/u,
      "Reserved-lineage proof staged runner resume/replace conflict gate",
    ],
    [
      "--negative-control-lineage-proof-log-exact",
      /test_lines != \[expected_test_line\][\s\S]*?False/u,
      "Reserved-lineage proof evidence exact proof-log gate",
    ],
    [
      "--negative-control-lineage-proof-log-size-limit",
      /max_bytes=MAX_LINEAGE_PROOF_LOG_BYTES[\s\S]*?max_bytes=None/u,
      "Reserved-lineage proof evidence proof-log size limit",
    ],
    [
      "--negative-control-lineage-proof-log-is-file-preflight",
      /actual_log_digest, log_errors = validate_lineage_proof_log\([\s\S]*?log_file_missing = log_errors == \["missing production proof log"\][\s\S]*?log_file_exists = log_artifact_path\.is_file\(\)/u,
      "Reserved-lineage proof evidence proof-log is_file preflight gate",
    ],
    [
      "--negative-control-lineage-proof-log-text-preflight",
      /text = b""\.join\(chunks\)\.decode\("utf-8", errors=decode_errors\)[\s\S]*?text = path\.read_text\(encoding="utf-8", errors=decode_errors\)/u,
      "Reserved-lineage proof evidence proof-log text preflight gate",
    ],
    [
      "--negative-control-lineage-proof-log-open-path-binding",
      /digest, text, read_errors = _sha256_text_file\([\s\S]*?"production proof log"[\s\S]*?_sha256_text_file_unbound\(/u,
      "Reserved-lineage proof evidence proof-log open-path binding",
    ],
    [
      "--negative-control-lineage-proof-evidence-filename",
      /lineage_proof_evidence_filename[\s\S]*?lineage_proof_evidence_any_filename/u,
      "Reserved-lineage proof evidence filename gate",
    ],
    [
      "--negative-control-lineage-proof-evidence-output-parent-sync-identity",
      /expected_identity=parent_identity[\s\S]*?expected_identity=None/u,
      "Reserved-lineage proof evidence output parent sync identity gate",
    ],
    [
      "--negative-control-lineage-proof-closed-schema",
      /lineage_proof_evidence_unexpected_field[\s\S]*?lineage_proof_evidence_allows_extra_fields/u,
      "Reserved-lineage proof evidence closed schema",
    ],
    [
      "--negative-control-lineage-proof-evidence-helper",
      /validate_lineage_proof_command[\s\S]*?lineage_proof_command_disabled/u,
      "Reserved-lineage proof evidence helper runtime-keygen guard",
    ],
    [
      "--negative-control-lineage-proof-timestamp-raw",
      /SIGNED_AT_UTC_RE\.fullmatch\(generated_at_raw\)[\s\S]*?SIGNED_AT_UTC_RE\.fullmatch\(generated_at_raw\.strip\(\)\)/u,
      "Reserved-lineage proof evidence raw timestamp gate",
    ],
    [
      "--negative-control-lineage-proof-readiness-direct-hash-shape",
      /_validate_lineage_local_file_for_read\(path, label\)[\s\S]*?return None, file_errors[\s\S]*?expected_stat = path\.stat\(\)/u,
      "Reserved-lineage proof readiness direct hash path-shape gate",
    ],
    [
      "--negative-control-lineage-proof-readiness-direct-hash-read-failure",
      /\{label\} could not be read[\s\S]*?except OSError:[\s\S]*?raise/u,
      "Reserved-lineage proof readiness direct hash read-failure gate",
    ],
    [
      "--negative-control-sdk-default",
      /if recursive_spend_available[\s\S]*?if _recursive_compact_available/u,
      "SDK default selector",
    ],
    [
      "--negative-control-pallas-envelope-type",
      /kagemusha_recursive_compact_record_prover_preflights_pallas_archive_before_unavailable[\s\S]*?kagemusha_recursive_compact_record_prover_skips_pallas_archive_before_unavailable/u,
      "ABI-7 compact Pallas envelope preflight type",
    ],
    [
      "--negative-control-staged-path-aliases",
      /staged_alias_checks[\s\S]*?kagemusha_run_lineage_proof_staged\.py[\s\S]*?kagemusha_run_recursive_compact_keygen_staged\.py[\s\S]*?kagemusha_finalize_lineage_proof_staged_run\.py[\s\S]*?kagemusha_finalize_recursive_compact_key_staged_run\.py/u,
      "Kagemusha staged path alias gate",
    ],
    [
      "--negative-control-android-device-lab-d2d-transport-matrix",
      new RegExp(
        [
          "if missing_transports:",
          "[\\s\\S]*?if False and missing_transports:",
          "[\\s\\S]*?if missing_d2d_payment_transports:",
          "[\\s\\S]*?if False and missing_d2d_payment_transports:",
          "[\\s\\S]*?if set\\(artifacts\\) != set\\(expected_artifacts\\):",
          "[\\s\\S]*?if False and set\\(artifacts\\) != set\\(expected_artifacts\\):",
        ].join(""),
        "u",
      ),
      "Android device-lab D2D transport matrix gate",
    ],
    [
      "--negative-control-android-release-bundle-d2d-declaration-binding",
      new RegExp(
        [
          "and d2d_transport not in d2d_transports",
          "[\\s\\S]*?and False",
          "[\\s\\S]*?and set\\(d2d_transcripts\\) != declared_d2d_transports",
          "[\\s\\S]*?and False",
          "[\\s\\S]*?elif d2d_transports_valid and d2d_transcripts is None:",
          "[\\s\\S]*?elif False and d2d_transcripts is None:",
        ].join(""),
        "u",
      ),
      "Android release-bundle D2D declaration binding gate",
    ],
    [
      "--negative-control-release-bundle-android-d2d-transcript-binding-shape",
      /if \([\s\S]*?not isinstance\(binding, dict\)[\s\S]*?if False and \([\s\S]*?not isinstance\(binding, dict\)/u,
      "Kagemusha release bundle Android D2D transcript binding shape",
    ],
    [
      "--negative-control-release-bundle-android-d2d-transport-list-shape",
      /not d2d_transports_all_strings[\s\S]*?False/u,
      "Kagemusha release bundle Android D2D transport list shape",
    ],
    [
      "--negative-control-compact-key-scalar-types",
      /not isinstance\(compact_scalar_value, int\)[\s\S]*?False/u,
      "ABI-7 compact key evidence scalar type gate",
    ],
    [
      "--negative-control-compact-key-timestamp-raw",
      /generated_at_raw = generated_at_text[\s\S]*?generated_at_stripped = generated_at_text\.strip\(\)/u,
      "ABI-7 compact key evidence timestamp raw gate",
    ],
    [
      "--negative-control-compact-key-evidence-filename",
      /compact_key_evidence_filename[\s\S]*?compact_key_evidence_any_filename/u,
      "ABI-7 compact key evidence filename gate",
    ],
    [
      "--negative-control-compact-key-closed-schema",
      /compact_key_evidence_unexpected_field[\s\S]*?compact_key_evidence_allows_extra_fields/u,
      "ABI-7 compact key evidence closed schema",
    ],
    [
      "--negative-control-android-signed-evidence-summary-identity-fields",
      /device_lab\.infer_kagemusha_device_family[\s\S]*?device_lab\.accept_any_kagemusha_device_family/u,
      "Android signed-evidence readiness summary identity binding",
    ],
    [
      "--negative-control-android-device-lab-artifact-binding",
      /signed_evidence_artifact_sha256 does not match signed_evidence_artifact_path[\s\S]*?signed_evidence_artifact_sha256 is accepted without matching signed_evidence_artifact_path/u,
      "Android device-lab signed-evidence artifact binding",
    ],
    [
      "--negative-control-android-device-lab-abi6-probe-status-exactness",
      /_require_status\(metadata, "abi6_recursive_spend_jni_probe", \{"passed"\}, errors\)[\s\S]*?_require_status\(metadata, "abi6_recursive_spend_jni_probe", \{"passed", "ok"\}, errors\)/u,
      "Android device-lab ABI-6 probe exact passed status gate",
    ],
    [
      "--negative-control-android-device-lab-ancestor-cwd-failure",
      /candidate = Path\.cwd\(\) \/ path[\s\S]*?return \[f"\{label\} metadata could not be read"\][\s\S]*?candidate = path if path\.is_absolute\(\) else Path\.cwd\(\) \/ path/u,
      "Android device-lab ancestor cwd failure gate",
    ],
    [
      "--negative-control-android-device-lab-ancestor-metadata-failure",
      /errors\.append\(f"\{label\} metadata could not be read"\)[\s\S]*?break[\s\S]*?continue/u,
      "Android device-lab ancestor metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-ancestor-is-symlink-preflight",
      /stat\.S_ISLNK\(ancestor_mode\)[\s\S]*?ancestor\.is_symlink\(\)/u,
      "Android device-lab ancestor is_symlink preflight gate",
    ],
    [
      "--negative-control-android-device-lab-ancestor-exists-preflight",
      /ancestor_mode = ancestor\.lstat\(\)\.st_mode[\s\S]*?if not ancestor\.exists\(\):[\s\S]*?ancestor_mode = ancestor\.stat\(\)\.st_mode/u,
      "Android device-lab ancestor exists preflight gate",
    ],
    [
      "--negative-control-android-device-lab-attestation-binding",
      /SIGNED_EVIDENCE_SLOT_SHA256_FIELDS: tuple\[str, \.\.\.\][\s\S]*?SIGNED_EVIDENCE_SLOT_OPTIONAL_SHA256_FIELDS: tuple\[str, \.\.\.\]/u,
      "Android device-lab attestation challenge binding",
    ],
    [
      "--negative-control-android-device-lab-attestation-chain-binding",
      /slot\.json attestation_certificate_chain_sha256 does not match attestation_certificate_chain_path[\s\S]*?slot\.json attestation_certificate_chain_sha256 may ignore attestation_certificate_chain_path/u,
      "Android device-lab attestation certificate-chain binding",
    ],
    [
      "--negative-control-android-device-lab-attestation-chain-shape",
      /attestation certificate chain PEM must contain certificate boundaries[\s\S]*?attestation certificate chain PEM may omit certificate boundaries/u,
      "Android device-lab attestation certificate-chain artifact shape",
    ],
    [
      "--negative-control-android-device-lab-attestation-slot-binding",
      /attestation\/result\.json \{slot_key\} must match the slot directory name[\s\S]*?attestation\/result\.json \{slot_key\} may differ from the slot directory name/u,
      "Android device-lab attestation slot binding",
    ],
    [
      "--negative-control-android-device-lab-attestation-schema",
      /set\(result\) - ATTESTATION_RESULT_FIELDS[\s\S]*?set\(\)/u,
      "Android device-lab attestation result schema",
    ],
    [
      "--negative-control-android-device-lab-attestation-report",
      /validate_attestation_report\(slot_path, metadata, errors\)[\s\S]*?validate_attestation_result\(slot_path, metadata, errors\)/u,
      "Android device-lab attestation verifier report gate",
    ],
    [
      "--negative-control-android-device-lab-attestation-report-level-fields",
      /"attestation_security_level",\\n[\s\S]*?"keymaster_security_level",\\n[\s\S]*?"keymint_security_level",\\n    \):\\n        value = _attestation_report_verification_string/u,
      "Android device-lab attestation verifier report level-field gate",
    ],
    [
      "--negative-control-android-device-lab-attestation-report-result-level-binding",
      /and result_level != report_level[\s\S]*?and False/u,
      "Android device-lab attestation verifier report/result level binding",
    ],
    [
      "--negative-control-android-device-lab-attestation-report-result-status-binding",
      /and result_status != report_status[\s\S]*?and False/u,
      "Android device-lab attestation verifier report/result status binding",
    ],
    [
      "--negative-control-android-device-lab-attestation-status-exactness",
      /if status is not None and status != "ok":[\s\S]*?if status is not None and status not in \{"ok", "passed"\}:/u,
      "Android device-lab attestation exact ok status gate",
    ],
    [
      "--negative-control-android-device-lab-attestation-result-slot-keymint-binding",
      /attestation\/result\.json keymint_security_level must match[\s\S]*?attestation\/result\.json keymint_security_level may differ from/u,
      "Android device-lab attestation result slot KeyMint binding",
    ],
    [
      "--negative-control-android-device-lab-capture-attestation-result-binding",
      /attestation result attestation_challenge_sha256 must match attestation\/challenge\.hex[\s\S]*?attestation result attestation_challenge_sha256 may differ from attestation\/challenge\.hex/u,
      "Android capture wrapper attestation-result binding",
    ],
    [
      "--negative-control-android-device-lab-capture-chain-binding",
      /attestation result attestation_certificate_chain_sha256 must match[\s\S]*?attestation result attestation_certificate_chain_sha256 may differ/u,
      "Android capture wrapper certificate-chain digest binding",
    ],
    [
      "--negative-control-android-device-lab-capture-summary-parent-sync-identity",
      /_file_identity\(current_parent_stat\) != parent_identity[\s\S]*?False/u,
      "Android capture summary parent sync identity gate",
    ],
    [
      "--negative-control-android-device-lab-capture-summary-published-cleanup-identity",
      /_file_identity\(path_stat\) == expected_identity[\s\S]*?True/u,
      "Android capture summary published cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-capture-summary-temp-cleanup-identity",
      /_file_identity\(path_stat\) == expected_identity[\s\S]*?True/u,
      "Android capture summary temp cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-cli-secret-paths",
      /--root must not contain secret-looking material[\s\S]*?--root may contain secret-looking material/u,
      "Android device-lab CLI secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-d2d-transcript",
      /d2d payment transcript queue_after_sha256 must match queue\/pending_queue\.json[\s\S]*?d2d payment transcript queue_after_sha256 may ignore queue\/pending_queue\.json/u,
      "Android device-lab D2D payment transcript binding",
    ],
    [
      "--negative-control-android-device-lab-d2d-path-root",
      /slot\.json d2d_payment_transcript_path must stay under handoff\/[\s\S]*?slot\.json d2d_payment_transcript_path may point outside handoff\//u,
      "Android device-lab D2D payment transcript handoff path root",
    ],
    [
      "--negative-control-android-device-lab-d2d-queue-is-file-preflight",
      /_metadata_artifact_bytes_and_sha256\([\s\S]*?"queue\/pending_queue\.json"[\s\S]*?queue_path = slot_path \/ "queue" \/ "pending_queue\.json"[\s\S]*?queue_path\.is_file\(\)/u,
      "Android device-lab D2D queue is_file preflight gate",
    ],
    [
      "--negative-control-android-device-lab-digest-artifact-file-metadata-failure",
      /def _slot_artifact_lstat_mode\([\s\S]*?except OSError:[\s\S]*?return None, \[metadata_error\][\s\S]*?return artifact_path\.lstat\(\)\.st_mode, \[\]/u,
      "Android device-lab digest artifact file metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-direct-helper-slot-secret-paths",
      /SECRET_RE\.search\(path_text\)[\s\S]*?slot path must not contain secret-looking material[\s\S]*?""/u,
      "Android device-lab direct helper slot secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-direct-helper-slot-path-aliases",
      /if "\\\\\\\\" in path_text:[\s\S]*?slot path must not contain backslashes[\s\S]*?""/u,
      "Android device-lab direct helper slot path-alias gate",
    ],
    [
      "--negative-control-android-device-lab-direct-symlink-artifact-slot-secret-paths",
      /def validate_no_slot_symlink_artifacts[\s\S]*?_reject_secret_slot_path\(slot_path, errors\)[\s\S]*?def validate_no_slot_symlink_artifacts[\s\S]*?Reject symlinked slot metadata/u,
      "Android device-lab direct symlink-artifact slot secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-direct-hardlink-artifact-slot-secret-paths",
      /def validate_no_slot_hardlink_artifacts[\s\S]*?_reject_secret_slot_path\(slot_path, errors\)[\s\S]*?def validate_no_slot_hardlink_artifacts[\s\S]*?Reject hardlinked slot metadata/u,
      "Android device-lab direct hardlink-artifact slot secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-direct-regular-artifact-slot-secret-paths",
      /def validate_slot_regular_file_artifacts[\s\S]*?_reject_secret_slot_path\(slot_path, errors\)[\s\S]*?def validate_slot_regular_file_artifacts[\s\S]*?Reject special-file slot metadata/u,
      "Android device-lab direct regular-artifact slot secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-discover-slots-is-dir-preflight",
      /stat\.S_ISDIR\(entry_mode\) or stat\.S_ISLNK\(entry_mode\)[\s\S]*?entry\.is_dir\(\)/u,
      "Android device-lab discover_slots is_dir preflight gate",
    ],
    [
      "--negative-control-android-device-lab-discover-slots-entry-metadata-failure",
      /device-lab slot directory metadata could not be read[\s\S]*?except OSError:[\s\S]*?continue/u,
      "Android device-lab discover_slots entry metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-duplicate-binding-zero-digest",
      /or value == "0" \* 64[\s\S]*?""/u,
      "Android device-lab duplicate-binding zero digest",
    ],
    [
      "--negative-control-android-device-lab-duplicate-json-keys",
      /object_pairs_hook=_reject_duplicate_json_object_pairs[\s\S]*?object_pairs_hook=dict/u,
      "Android device-lab duplicate JSON key gate",
    ],
    [
      "--negative-control-android-device-lab-hardlink-artifacts",
      /sha256sum\.txt references hardlinked artifact[\s\S]*?sha256sum\.txt accepts hardlinked artifact/u,
      "Android device-lab hardlink artifact gate",
    ],
    [
      "--negative-control-android-device-lab-hardlink-artifact-metadata-failure",
      /def _reject_hardlinked_file[\s\S]*?except OSError:[\s\S]*?file metadata could not be read[\s\S]*?if path\.is_symlink\(\) or not path\.is_file\(\):/u,
      "Android device-lab hardlink artifact metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-hardlink-artifact-directory-exists-preflight",
      /dir_mode = dir_path\.lstat\(\)\.st_mode[\s\S]*?stat\.S_ISLNK\(dir_mode\) or not stat\.S_ISDIR\(dir_mode\)[\s\S]*?dir_path\.is_symlink\(\) or not dir_path\.exists\(\)/u,
      "Android device-lab hardlink artifact directory exists preflight gate",
    ],
    [
      "--negative-control-android-device-lab-incomplete-slot-coverage",
      /and _android_report_has_complete_signed_evidence\(report, signed_evidence\)[\s\S]*?and True/u,
      "Android device-lab incomplete slot matrix coverage",
    ],
    [
      "--negative-control-android-device-lab-instrumentation-harness",
      /OfflineNoteTransferHandoffTest\.java[\s\S]*?nearbyQrAndNfcTokenHandoffRoundTripFixtureBytes[\s\S]*?qrAndNfcTokenHandoffRoundTripDisabled/u,
      "Android device-lab instrumentation harness",
    ],
    [
      "--negative-control-android-device-lab-json-load-ancestor",
      /json_ancestor_errors = validate_no_symlink_ancestors\([\s\S]*?json_ancestor_errors = _skip_json_ancestor_validation\(/u,
      "Android device-lab JSON loader ancestor symlink gate",
    ],
    [
      "--negative-control-android-device-lab-json-load-direct-secret-paths",
      /SECRET_RE\.search\(path_text\)[\s\S]*?\{label\} path must not contain secret-looking material[\s\S]*?""/u,
      "Android device-lab JSON loader direct secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-json-load-direct-control-paths",
      /_contains_control_character\(path_text\)[\s\S]*?\{label\} path must not contain control characters[\s\S]*?""/u,
      "Android device-lab JSON loader direct control-path gate",
    ],
    [
      "--negative-control-android-device-lab-json-load-direct-path-aliases",
      /if "\\\\\\\\" in path_text:[\s\S]*?\{label\} path must not contain backslashes[\s\S]*?""/u,
      "Android device-lab JSON loader direct path-alias gate",
    ],
    [
      "--negative-control-android-device-lab-json-load-file-metadata-failure",
      /expected_stat = path\.lstat\(\)[\s\S]*?except OSError:[\s\S]*?\{label\} file metadata could not be read[\s\S]*?expected_stat = path\.lstat\(\)/u,
      "Android device-lab JSON loader file metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-json-load-size-limit",
      /open_stat\.st_size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES[\s\S]*?False/u,
      "Android device-lab JSON loader size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-json-load-read-failure",
      /except \(OSError, UnicodeDecodeError\):[\s\S]*?\{label\} could not be read[\s\S]*?""/u,
      "Android device-lab JSON loader read/decode failure gate",
    ],
    [
      "--negative-control-android-device-lab-json-load-open-path-binding",
      /json_expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?json_expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Android device-lab JSON loader open-path binding gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-aliases",
      /\{label\} must not be a symlink[\s\S]*?\{label\} may be a symlink/u,
      "Android device-lab JSON summary output alias gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-direct-secret-paths",
      /SECRET_RE\.search\(path_text\)[\s\S]*?\{label\} must not contain secret-looking material[\s\S]*?""/u,
      "Android device-lab direct JSON summary output secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-direct-control-paths",
      /_contains_control_character\(path_text\)[\s\S]*?\{label\} must not contain control characters[\s\S]*?""/u,
      "Android device-lab direct JSON summary output control-path gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-direct-path-aliases",
      /if "\\\\\\\\" in path_text:[\s\S]*?\{label\} must not contain backslashes[\s\S]*?""/u,
      "Android device-lab direct JSON summary output path-alias gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-file-metadata-failure",
      /output_mode = path\.lstat\(\)\.st_mode[\s\S]*?\{label\} file metadata could not be read[\s\S]*?except FileNotFoundError:[\s\S]*?return \[\]/u,
      "Android device-lab JSON summary output file metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-hardlink-metadata-failure",
      /link_count = path\.stat\(\)\.st_nlink[\s\S]*?\{label\} hardlink metadata could not be read[\s\S]*?link_count = path\.stat\(\)\.st_nlink/u,
      "Android device-lab JSON summary output hardlink metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-parent-create-failure",
      /parent\.mkdir\(parents=True, exist_ok=True\)[\s\S]*?\{label\} parent directory could not be created[\s\S]*?parent\.mkdir\(parents=True, exist_ok=True\)/u,
      "Android device-lab JSON summary output parent-create failure gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-parent-is-dir-preflight",
      /not stat\.S_ISDIR\(parent_mode\)[\s\S]*?not parent\.is_dir\(\)/u,
      "Android device-lab JSON summary output parent is_dir preflight gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-parent-metadata-failure",
      /\{label\} parent directory metadata could not be read[\s\S]*?return False, \[\]/u,
      "Android device-lab JSON summary output parent metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-post-create-parent-preflight",
      /_validate_summary_output_parent\([\s\S]*?\{label\} parent must be a directory[\s\S]*?""/u,
      "Android device-lab JSON summary output post-create parent preflight gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-parent-sync-identity",
      /_file_identity\(parent_fd_stat\) != parent_identity[\s\S]*?_file_identity\(current_parent_stat\) != parent_identity[\s\S]*?expected_identity=parent_identity[\s\S]*?expected_identity=None/u,
      "Android device-lab JSON summary output parent sync identity gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-post-write-preflight",
      /validate_summary_output_path\(path, "--json-out"\)[\s\S]*?expected_stat = path\.lstat\(\)/u,
      "Android device-lab JSON summary output post-write preflight gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-published-cleanup-identity",
      /_file_identity\(output_stat\) != expected_identity[\s\S]*?False/u,
      "Android device-lab JSON summary output published cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-published-cleanup-report",
      /write_errors\.extend\(\[\*sync_errors, \*cleanup_errors\]\)[\s\S]*?write_errors\.extend\(sync_errors\)/u,
      "Android device-lab JSON summary output published cleanup report gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-readback-verification",
      /readback_text != summary_text[\s\S]*?False/u,
      "Android device-lab JSON summary output readback gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-readback-failure",
      /--json-out write verification failed[\s\S]*?return None, \[\]/u,
      "Android device-lab JSON summary output readback failure gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-readback-size-limit",
      /open_stat\.st_size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES[\s\S]*?--json-out must be no more than[\s\S]*?""/u,
      "Android device-lab JSON summary output readback size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-readback-open-path-binding",
      /summary_expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?summary_expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Android device-lab JSON summary output readback open-path binding gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-size-limit",
      /len\(summary_text\.encode\("utf-8"\)\) > MAX_ANDROID_DEVICE_LAB_JSON_BYTES[\s\S]*?--json-out must be no more than[\s\S]*?""/u,
      "Android device-lab JSON summary output size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-strict-json-write",
      /json\.dumps\(summary, indent=2, allow_nan=False\)[\s\S]*?json\.dumps\(summary, indent=2\)/u,
      "Android device-lab JSON summary strict JSON write gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-temp-cleanup-failure",
      /--json-out temporary file could not be removed[\s\S]*?return \[\]/u,
      "Android device-lab JSON summary output temp cleanup failure gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-temp-cleanup-identity",
      /_file_identity\(temp_stat\) != expected_identity[\s\S]*?False/u,
      "Android device-lab JSON summary output temp cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-write-failure",
      /os\.replace\([\s\S]*?src_dir_fd=parent_fd[\s\S]*?dst_dir_fd=parent_fd[\s\S]*?path\.write_text\(summary_text, encoding="utf-8"\)/u,
      "Android device-lab JSON summary output write-failure gate",
    ],
    [
      "--negative-control-android-device-lab-main-root-exists-preflight",
      /if not root_exists:[\s\S]*?if args\.allow_missing_root:[\s\S]*?if not root\.exists\(\):/u,
      "Android device-lab scanner root exists preflight gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-artifact-digest-preflight",
      /_validate_manifest_artifact_for_digest\([\s\S]*?assert artifact_path is not None and artifact_stat is not None[\s\S]*?artifact_stat = artifact_path\.lstat\(\)/u,
      "Android device-lab manifest artifact digest preflight gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-artifact-open-path-binding",
      /manifest_expected_identity = \([\s\S]*?expected_stat\.st_dev[\s\S]*?expected_stat\.st_ino[\s\S]*?manifest_expected_identity = \([\s\S]*?open_stat\.st_dev[\s\S]*?open_stat\.st_ino/u,
      "Android device-lab manifest artifact open-path binding gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-artifact-read-failure",
      /sha256sum\.txt references artifact that could not be read[\s\S]*?return None, \[\]/u,
      "Android device-lab manifest artifact digest read-failure gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-artifact-size-limit",
      /open_stat\.st_size > max_bytes[\s\S]*?size > max_bytes[\s\S]*?False/u,
      "Android device-lab manifest artifact size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-file-metadata-failure",
      /manifest_stat = manifest_path\.lstat\(\)[\s\S]*?sha256sum\.txt file metadata could not be read[\s\S]*?missing sha256sum\.txt/u,
      "Android device-lab manifest file metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-file-shape-terminal",
      /_has_manifest_file_shape_error\(errors\)[\s\S]*?return errors[\s\S]*?""/u,
      "Android device-lab manifest file-shape terminal gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-hardlink",
      /manifest_path\.stat\(\)\.st_nlink > 1[\s\S]*?sha256sum\.txt must not be hardlinked[\s\S]*?sha256sum\.txt hardlink metadata could not be read[\s\S]*?""/u,
      "Android device-lab manifest hardlink gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-hardlink-metadata-failure",
      /manifest_path\.stat\(\)\.st_nlink > 1[\s\S]*?sha256sum\.txt hardlink metadata could not be read[\s\S]*?manifest_path\.stat\(\)\.st_nlink > 1/u,
      "Android device-lab manifest hardlink metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-open-path-binding",
      /expected_identity = \(manifest_stat\.st_dev, manifest_stat\.st_ino\)[\s\S]*?expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Android device-lab manifest open-path binding gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-parse-direct-slot-secret-paths",
      /root_errors = _validate_manifest_slot_path\(slot_path\)[\s\S]*?return entries, root_errors[\s\S]*?""/u,
      "Android device-lab manifest parser direct slot secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-read-failure",
      /except \(OSError, UnicodeDecodeError\):[\s\S]*?sha256sum\.txt could not be read[\s\S]*?except UnicodeDecodeError:/u,
      "Android device-lab manifest read/decode failure gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-size-limit",
      /open_stat\.st_size > MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES[\s\S]*?False/u,
      "Android device-lab manifest size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-slot-ancestor-symlink",
      /validate_no_symlink_ancestors\(slot_path, "slot ancestor directory"\)[\s\S]*?return \[\]/u,
      "Android device-lab manifest slot-ancestor symlink gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-slot-metadata-failure",
      /slot directory metadata could not be read[\s\S]*?slot_mode = None/u,
      "Android device-lab manifest slot metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-slot-root-symlink",
      /stat\.S_ISLNK\(slot_mode\)[\s\S]*?slot directory must not be a symlink[\s\S]*?""/u,
      "Android device-lab manifest slot-root symlink gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-verify-direct-slot-secret-paths",
      /root_errors = _validate_manifest_slot_path\(slot_path\)[\s\S]*?return root_errors[\s\S]*?""/u,
      "Android device-lab manifest verifier direct slot secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-manifest-verify-symlink-directory",
      /_slot_relative_symlink_ancestor\(slot_path, safe_relative\)[\s\S]*?sha256sum\.txt references artifact under symlink directory[\s\S]*?artifact_path = slot_path \/ safe_relative/u,
      "Android device-lab manifest verifier symlink directory gate",
    ],
    [
      "--negative-control-android-device-lab-slot-files-direct-root-shape",
      /def _slot_files\(slot_path: Path, errors: list\[str\] \| None = None\) -> set\[str\]:[\s\S]*?_slot_path_boundary_errors\(slot_path\)[\s\S]*?def _slot_files\(slot_path: Path, errors: list\[str\] \| None = None\) -> set\[str\]:/u,
      "Android device-lab slot file discovery direct root-shape gate",
    ],
    [
      "--negative-control-android-device-lab-slot-files-root-metadata-failure",
      /slot directory metadata could not be read[\s\S]*?return set\(\)/u,
      "Android device-lab slot file discovery root metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-slot-files-direct-secret-paths",
      /_slot_path_boundary_errors\(slot_path\)[\s\S]*?slot_errors\.extend\(path_errors\)[\s\S]*?path_errors = \[\]/u,
      "Android device-lab slot file discovery direct secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-slot-files-direct-ancestor-symlink",
      /validate_no_symlink_ancestors\(slot_path, "slot ancestor directory"\)[\s\S]*?""/u,
      "Android device-lab slot file discovery ancestor symlink gate",
    ],
    [
      "--negative-control-android-device-lab-slot-files-direct-symlink-directory",
      /dir_path\.lstat\(\)\.st_mode[\s\S]*?\{dirname\}\/ metadata could not be read[\s\S]*?dir_path\.is_dir\(\)/u,
      "Android device-lab slot file discovery symlink directory gate",
    ],
    [
      "--negative-control-android-device-lab-slot-files-directory-metadata-failure",
      /\{dirname\}\/ metadata could not be read[\s\S]*?except OSError:[\s\S]*?continue/u,
      "Android device-lab slot file discovery directory metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-slot-top-level-listing-failure",
      /def _slot_root_entries\(slot_path: Path, errors: list\[str\]\)[\s\S]*?slot directory could not be listed[\s\S]*?return sorted\(slot_path\.iterdir\(\), key=lambda entry: entry\.name\)/u,
      "Android device-lab slot top-level listing failure gate",
    ],
    [
      "--negative-control-android-device-lab-slot-files-artifact-metadata-failure",
      /entry\.lstat\(\)\.st_mode[\s\S]*?slot artifact \{_display_path\(relative\)\} file metadata could not be read[\s\S]*?entry\.is_file\(\) or entry\.is_symlink\(\)/u,
      "Android device-lab slot file discovery artifact metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-slot-dir-symlink",
      /slot directory must not be a symlink[\s\S]*?slot directory may be a symlink/u,
      "Android device-lab slot directory symlink gate",
    ],
    [
      "--negative-control-android-device-lab-slot-metadata-failure",
      /slot directory metadata could not be read[\s\S]*?slot_mode = None/u,
      "Android device-lab slot metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-slot-parent-symlink",
      /slot parent directory must not be a symlink[\s\S]*?slot parent directory may be a symlink/u,
      "Android device-lab slot parent symlink gate",
    ],
    [
      "--negative-control-android-device-lab-slot-parent-metadata-failure",
      /slot parent directory metadata could not be read[\s\S]*?parent_mode = None/u,
      "Android device-lab slot parent metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-slot-ancestor-symlink",
      /validate_no_symlink_ancestors\([\s\S]*?slot ancestor directory[\s\S]*?return ancestor_errors[\s\S]*?""/u,
      "Android device-lab slot ancestor symlink gate",
    ],
    [
      "--negative-control-android-device-lab-slot-directory-traversal-failure",
      /\{label\} could not be listed[\s\S]*?\{label\} listing failures ignored/u,
      "Android device-lab slot directory traversal-failure gate",
    ],
    [
      "--negative-control-android-device-lab-slot-regular-file-metadata-failure",
      /path\.lstat\(\)\.st_mode[\s\S]*?\{label\} file metadata could not be read[\s\S]*?mode = path\.lstat\(\)\.st_mode/u,
      "Android device-lab slot regular-file metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-slot-regular-file-exists-preflight",
      /def _reject_non_regular_file\(path: Path, label: str, errors: list\[str\]\) -> None:[\s\S]*?path\.is_symlink\(\) or not path\.exists\(\)/u,
      "Android device-lab slot regular-file exists preflight gate",
    ],
    [
      "--negative-control-android-device-lab-slot-directory-metadata-failure",
      /dir_path\.lstat\(\)\.st_mode[\s\S]*?\{dirname\}\/ metadata could not be read[\s\S]*?mode = dir_path\.lstat\(\)\.st_mode/u,
      "Android device-lab slot directory metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-slot-directory-exists-preflight",
      /dir_path\.lstat\(\)\.st_mode[\s\S]*?stat\.S_ISLNK\(mode\)[\s\S]*?dir_path\.is_symlink\(\) or not dir_path\.exists\(\)/u,
      "Android device-lab slot directory exists preflight gate",
    ],
    [
      "--negative-control-android-device-lab-slot-artifact-file-metadata-failure",
      /entry\.lstat\(\)\.st_mode[\s\S]*?slot artifact \{_display_path\(relative\)\} file metadata could not be read[\s\S]*?entry_mode = entry\.lstat\(\)\.st_mode/u,
      "Android device-lab slot artifact file metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-slot-artifact-symlink-preflight",
      /for entry in entries:[\s\S]*?stat\.S_ISLNK\(entry_mode\)[\s\S]*?if entry\.is_symlink\(\):/u,
      "Android device-lab slot artifact symlink preflight gate",
    ],
    [
      "--negative-control-android-device-lab-slot-id-safety",
      /validate_slot_ids\(args\.slots\)[\s\S]*?args\.slots, \[\]/u,
      "Android device-lab explicit slot id safety",
    ],
    [
      "--negative-control-android-device-lab-slot-name-safety",
      /slot directory name must not contain secret-looking material[\s\S]*?slot directory name may contain secret-looking material[\s\S]*?slot directory name must not contain whitespace[\s\S]*?slot directory name may contain whitespace[\s\S]*?slot directory name must not contain control characters[\s\S]*?slot directory name may contain control characters/u,
      "Android device-lab discovered slot name safety",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-signature-required",
      /signing inputs are required unless --allow-unsigned is set[\s\S]*?signing inputs are optional by default/u,
      "Android device-lab slot assembler signing-required gate",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-family-override-binding",
      /has_device_identity and inferred != family[\s\S]*?device family must match attached device model\/codename[\s\S]*?""/u,
      "Android device-lab slot assembler requested-family binding gate",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-device-identity-fields",
      /"device_model": facts\["device_model"\][\s\S]*?"device_codename": facts\["device_codename"\][\s\S]*?"device_model": family[\s\S]*?"device_codename": family/u,
      "Android device-lab slot assembler device identity fields",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-harness-canonical",
      /level is not None and level not in device_lab\.STRONGBOX_LEVELS[\s\S]*?level\.upper\(\) not in device_lab\.STRONGBOX_LEVELS/u,
      "Android device-lab slot assembler harness canonical string gate",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-report-app-package-binding",
      /result_app_package != report_app_package[\s\S]*?and False/u,
      "Android device-lab slot assembler report app-package binding",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-result-closed-schema",
      /set\(attestation_result\) - device_lab\.ATTESTATION_RESULT_FIELDS[\s\S]*?set\(\)/u,
      "Android device-lab slot assembler attestation result closed schema",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-report-closed-schema",
      /set\(attestation_report\) - device_lab\.ATTESTATION_REPORT_FIELDS[\s\S]*?set\(\)/u,
      "Android device-lab slot assembler attestation report closed schema",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-report-verification-closed-schema",
      /set\(verification\) - device_lab\.ATTESTATION_REPORT_VERIFICATION_FIELDS[\s\S]*?set\(\)/u,
      "Android device-lab slot assembler attestation report verification closed schema",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-report-schema",
      /report_schema != device_lab\.ATTESTATION_REPORT_SCHEMA[\s\S]*?if False:/u,
      "Android device-lab slot assembler attestation report schema",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-report-verifier",
      /_require_source_string\(attestation_report, "verifier", "attestation\/report\.json", errors\)[\s\S]*?# unchecked attestation report verifier/u,
      "Android device-lab slot assembler attestation report verifier",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-d2d-closed-schema",
      /set\(d2d_payment_transcript\) - device_lab\.D2D_PAYMENT_TRANSCRIPT_FIELDS[\s\S]*?set\(\)/u,
      "Android device-lab slot assembler D2D transcript closed schema",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-wallet-closed-schema",
      /set\(wallet_integrity_transcript\) - device_lab\.WALLET_INTEGRITY_TRANSCRIPT_FIELDS[\s\S]*?set\(\)/u,
      "Android device-lab slot assembler wallet transcript closed schema",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-d2d-schema",
      /d2d_schema != device_lab\.D2D_PAYMENT_TRANSCRIPT_SCHEMA[\s\S]*?if False:/u,
      "Android device-lab slot assembler D2D transcript schema",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-wallet-schema",
      /wallet_schema != device_lab\.WALLET_INTEGRITY_TRANSCRIPT_SCHEMA[\s\S]*?if False:/u,
      "Android device-lab slot assembler wallet transcript schema",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-d2d-semantic-validation",
      /validate_d2d_payment_transcripts_binding\([\s\S]*?unchecked_d2d_payment_transcripts_binding\(/u,
      "Android device-lab slot assembler D2D transcript semantic validation",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-wallet-semantic-validation",
      /validate_wallet_integrity_transcript\([\s\S]*?unchecked_wallet_integrity_transcript\(/u,
      "Android device-lab slot assembler wallet transcript semantic validation",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-required-artifact-validation",
      /validate_required_kagemusha_slot_artifact_shapes\([\s\S]*?unchecked_required_kagemusha_slot_artifact_shapes\(/u,
      "Android device-lab slot assembler required artifact validation",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-report-level-binding",
      /result_level != report_level[\s\S]*?and False/u,
      "Android device-lab slot assembler report level binding",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-report-status-binding",
      /result_status != report_status[\s\S]*?and False/u,
      "Android device-lab slot assembler report status binding",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-attestation-status-exactness",
      /result_status is not None and result_status != "ok"[\s\S]*?result_status is not None and result_status not in \{"ok", "passed"\}[\s\S]*?report_status is not None and report_status != "ok"[\s\S]*?report_status is not None and report_status not in \{"ok", "passed"\}/u,
      "Android device-lab slot assembler exact ok status gate",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-source-open-binding",
      /open_identity != expected_identity or path_identity != expected_identity[\s\S]*?False/u,
      "Android device-lab slot assembler source open-path binding",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-root-path-aliases",
      /device-lab root path must not contain backslashes[\s\S]*?""/u,
      "Android device-lab slot assembler root path-alias gate",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-source-path-aliases",
      /\{label\} path must not contain backslashes[\s\S]*?""/u,
      "Android device-lab slot assembler source path-alias gate",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-copy-parent-sync-identity",
      /destination_parent_fd_stat[\s\S]*?destination_parent_identity[\s\S]*?or False[\s\S]*?current_destination_parent_stat[\s\S]*?False[\s\S]*?expected_identity=destination_parent_identity[\s\S]*?expected_identity=None/u,
      "Android device-lab slot assembler copy parent sync identity",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-published-cleanup-identity",
      /_file_identity\(output_stat\) != expected_identity[\s\S]*?False/u,
      "Android device-lab slot assembler published cleanup identity",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-published-cleanup-report",
      /cleanup_errors = _unlink_file_if_identity_at\([\s\S]*?rollback_blockers = _unlink_file_if_identity_at\(/u,
      "Android device-lab slot assembler published cleanup report",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-copy-readback",
      /if verify_errors:[\s\S]*?if False:/u,
      "Android device-lab slot assembler copy readback",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-json-parent-sync-identity",
      /parent_fd_stat[\s\S]*?json_parent_identity[\s\S]*?False[\s\S]*?current_parent_stat[\s\S]*?json_parent_identity[\s\S]*?False[\s\S]*?expected_identity=json_parent_identity[\s\S]*?expected_identity=None/u,
      "Android device-lab slot assembler JSON parent sync identity",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-json-readback",
      /return _verify_written_bytes\(path, encoded, label\)[\s\S]*?return \[\]/u,
      "Android device-lab slot assembler JSON readback",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-json-temp-cleanup-identity",
      /_file_identity\(temp_stat\) != expected_identity[\s\S]*?False/u,
      "Android device-lab slot assembler JSON temp cleanup identity",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-publish-root-identity",
      /_file_identity\(root_stat\) != expected_root_identity[\s\S]*?False/u,
      "Android device-lab slot assembler publish root identity",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-publish-stage-identity",
      /_file_identity\(stage_stat\) != expected_stage_identity[\s\S]*?False/u,
      "Android device-lab slot assembler publish staged-slot identity",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-temp-cleanup-identity",
      /_file_identity\(temp_parent_stat\) != expected_identity[\s\S]*?False/u,
      "Android device-lab slot assembler temporary cleanup identity",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-temp-cleanup-report",
      /if stage_errors or cleanup_errors:[\s\S]*?if stage_errors:/u,
      "Android device-lab slot assembler temporary cleanup report",
    ],
    [
      "--negative-control-android-device-lab-test-workflow",
      /check_android_device_lab_slot_test\.py[\s\S]*?disabled_check_android_device_lab_slot_test\.py/u,
      "Android device-lab validator workflow",
    ],
    [
      "--negative-control-android-device-lab-wallet-integrity",
      /wallet integrity transcript stale_snapshot_rejected must be true[\s\S]*?wallet integrity transcript stale_snapshot_rejected may be false/u,
      "Android device-lab wallet integrity transcript binding",
    ],
    [
      "--negative-control-android-device-lab-unique-bindings",
      /Android device-lab production slots must not reuse a device fingerprint[\s\S]*?Android device-lab production slots may reuse a device fingerprint/u,
      "Android device-lab unique matrix bindings",
    ],
    [
      "--negative-control-android-device-lab-summary",
      /covered_device_families[\s\S]*?covered_device_family_labels/u,
      "Android device-lab Kagemusha summary binding",
    ],
    [
      "--negative-control-android-device-lab-summary-complete-evidence",
      /require_complete_signed_evidence=require_complete_kagemusha[\s\S]*?require_complete_signed_evidence=False/u,
      "Android device-lab complete signed-evidence summary gate",
    ],
    [
      "--negative-control-android-device-lab-summary-trusted-signer-binding",
      /signer_public_key_sha256 not in trusted_signer_public_key_sha256[\s\S]*?and False/u,
      "Android device-lab summary trusted-signer binding",
    ],
    [
      "--negative-control-android-device-lab-summary-zero-trusted-signer-digest",
      /value != "0" \* 64[\s\S]*?SHA256_HEX_RE\.fullmatch\(value\) is not None/u,
      "Android device-lab summary zero trusted-signer digest",
    ],
    [
      "--negative-control-android-device-lab-trusted-signer-map-path-type",
      /trusted signer public key path must be a pathlib Path[\s\S]*?""/u,
      "Android device-lab trusted-signer direct-map path type",
    ],
    [
      "--negative-control-android-device-lab-trusted-signer-map-container",
      /trusted signer public key map must be a mapping[\s\S]*?""/u,
      "Android device-lab trusted-signer direct-map container",
    ],
    [
      "--negative-control-android-device-lab-trusted-signer-map-mixed-key-sort",
      /key=_trusted_signer_digest_sort_key[\s\S]*?key=None/u,
      "Android device-lab trusted-signer direct-map mixed-key sorting",
    ],
    [
      "--negative-control-android-device-lab-symlink-artifacts",
      /sha256sum\.txt references symlink artifact[\s\S]*?sha256sum\.txt accepts symlink artifact/u,
      "Android device-lab symlink artifact gate",
    ],
    [
      "--negative-control-android-device-lab-symlink-artifact-leaf-metadata-failure",
      /\{relative\} file metadata could not be read[\s\S]*?except OSError:[\s\S]*?continue/u,
      "Android device-lab symlink artifact leaf metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-symlink-artifact-directory-metadata-failure",
      /\{dirname\}\/ metadata could not be read[\s\S]*?if stat\.S_ISLNK\(dir_mode\)[\s\S]*?except OSError:[\s\S]*?continue[\s\S]*?if stat\.S_ISLNK\(dir_mode\)/u,
      "Android device-lab symlink artifact directory metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-symlink-artifact-nested-metadata-failure",
      /slot artifact \{_display_path\(relative\)\} file metadata could not be read[\s\S]*?if stat\.S_ISLNK\(entry_mode\)/u,
      "Android device-lab symlink artifact nested metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-telemetry-closed-schema",
      /telemetry\/telemetry\.json contains unexpected field[\s\S]*?telemetry\/telemetry\.json ignores unexpected field/u,
      "Android device-lab telemetry closed schema gate",
    ],
    [
      "--negative-control-android-device-lab-telemetry-identity-exactness",
      /_validate_telemetry_string[\s\S]*?_unchecked_telemetry_string/u,
      "Android device-lab telemetry identity exactness gate",
    ],
    [
      "--negative-control-android-device-lab-telemetry-app-package-binding",
      /telemetry\/telemetry\.json app_package_name must match [\s\S]*?telemetry\/telemetry\.json app_package_name may differ from /u,
      "Android device-lab telemetry app-package binding gate",
    ],
    [
      "--negative-control-android-device-lab-status-event-closed-schema",
      /telemetry\/status\.ndjson line \{line_no\} contains unexpected field[\s\S]*?telemetry\/status\.ndjson line \{line_no\} ignores unexpected field/u,
      "Android device-lab status event closed schema gate",
    ],
    [
      "--negative-control-android-device-lab-status-value-closed-schema",
      /telemetry\/status\.ndjson line \{line_no\} status must be ok[\s\S]*?telemetry\/status\.ndjson line \{line_no\} status may be advisory/u,
      "Android device-lab status value closed schema gate",
    ],
    [
      "--negative-control-android-device-lab-status-slot-binding-required",
      /telemetry\/status\.ndjson line \{line_no\} slot_id must be a non-empty string[\s\S]*?telemetry\/status\.ndjson line \{line_no\} slot_id may be omitted/u,
      "Android device-lab status slot binding required gate",
    ],
    [
      "--negative-control-android-device-lab-transcript-artifact-digest-preflight",
      /slot\.json d2d_payment_transcript_path[\s\S]*?unchecked_d2d_payment_transcript_path/u,
      "Android device-lab transcript artifact digest preflight gate",
    ],
    [
      "--negative-control-android-device-lab-staged-bytes-open-path-binding",
      /staged_expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?staged_expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Android device-lab staged bytes open-path binding gate",
    ],
    [
      "--negative-control-android-device-lab-staged-bytes-hardlink-readback",
      /open_stat\.st_nlink > 1[\s\S]*?verification_error[\s\S]*?""/u,
      "Android device-lab staged bytes hardlink readback gate",
    ],
    [
      "--negative-control-android-device-matrix",
      /ABI 7 recursive compact prover calls that require multi-hop append-batch[\s\S]*?ABI 7 recursive compact prover calls may be accepted as production state/u,
      "Android device-matrix compact one-hop/multi-hop boundary",
    ],
    [
      "--negative-control-android-signed-evidence-freshness-report",
      /_android_report_kagemusha\(report\)\.get\("signed_at_utc"\)[\s\S]*?_android_report_kagemusha\(report\)\.get\("unchecked_signed_at_utc"\)/u,
      "Android signed-evidence freshness report binding",
    ],
    [
      "--negative-control-android-signed-evidence-timestamp-raw",
      /SIGNED_AT_UTC_RE\.fullmatch\(signed_at_text\) is None[\s\S]*?SIGNED_AT_UTC_RE\.fullmatch\(signed_at_text\.strip\(\)\) is None/u,
      "Android signed-evidence report raw timestamp gate",
    ],
    [
      "--negative-control-android-signed-evidence-summary-partial-identity",
      /identity_fields and identity_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS[\s\S]*?False and identity_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS/u,
      "Android signed-evidence readiness summary partial identity omission",
    ],
    [
      "--negative-control-android-signed-evidence-summary-partial-artifact-binding",
      /artifact_fields and artifact_fields != expected[\s\S]*?False and artifact_fields != expected/u,
      "Android signed-evidence readiness summary partial artifact binding omission",
    ],
    [
      "--negative-control-android-signed-evidence-summary-partial-core-binding",
      /core_fields and core_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_CORE_FIELDS[\s\S]*?False and core_fields != ANDROID_SIGNED_EVIDENCE_SUMMARY_CORE_FIELDS/u,
      "Android signed-evidence readiness summary partial core binding omission",
    ],
    [
      "--negative-control-android-signed-evidence-summary-incomplete-entry",
      /set\(entry\) != ANDROID_SIGNED_EVIDENCE_SUMMARY_TARGET_FIELDS[\s\S]*?False and set\(entry\) != ANDROID_SIGNED_EVIDENCE_SUMMARY_TARGET_FIELDS/u,
      "Android signed-evidence readiness summary incomplete entry omission",
    ],
    [
      "--negative-control-android-signed-evidence-summary-slot-id",
      /safe_slot is None[\s\S]*?False and safe_slot is None/u,
      "Android signed-evidence readiness summary safe slot id gate",
    ],
    [
      "--negative-control-android-slot-summary-incomplete-kagemusha",
      /not _android_report_has_complete_signed_evidence\(report, signed_evidence\)[\s\S]*?False and not _android_report_has_complete_signed_evidence\(report, signed_evidence\)/u,
      "Android device-lab incomplete slot Kagemusha summary omission",
    ],
    [
      "--negative-control-android-duplicate-bindings-incomplete-slot-summary",
      /"duplicate_bindings": _android_duplicate_matrix_bindings_summary\([\s\S]*?"duplicate_bindings": device_lab\.kagemusha_duplicate_matrix_bindings\(reports\)/u,
      "Android duplicate-bindings summary complete-slot gate",
    ],
    [
      "--negative-control-android-device-lab-metadata-artifact-digest-preflight",
      /stat\.S_ISLNK\(artifact_stat\.st_mode\)[\s\S]*?not stat\.S_ISREG\(artifact_stat\.st_mode\)[\s\S]*?""/u,
      "Android device-lab metadata artifact digest preflight gate",
    ],
    [
      "--negative-control-android-device-lab-metadata-artifact-open-path-binding",
      /expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Android device-lab metadata artifact open-path binding gate",
    ],
    [
      "--negative-control-android-device-lab-metadata-artifact-read-failure",
      /return None, \[unreadable_error\][\s\S]*?return None, \[\]/u,
      "Android device-lab metadata artifact digest read-failure gate",
    ],
    [
      "--negative-control-android-device-lab-metadata-artifact-size-limit",
      /open_stat\.st_size > max_bytes[\s\S]*?size > max_bytes[\s\S]*?False/u,
      "Android device-lab metadata artifact digest size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-minimum-os",
      /slot\.json minimum_os for \{family\} must be[\s\S]*?slot\.json unsupported_os for \{family\} must be/u,
      "Android device-lab family minimum OS binding",
    ],
    [
      "--negative-control-android-device-lab-nonfinite-json-constants",
      /parse_constant=_reject_nonfinite_json_constant[\s\S]*?parse_constant=float/u,
      "Android device-lab non-finite JSON constant gate",
    ],
    [
      "--negative-control-android-device-lab-pending-queue-shape",
      /_validate_required_pending_queue_artifact\(slot_path, errors\)[\s\S]*?# unchecked pending queue shape/u,
      "Android device-lab pending queue shape gate",
    ],
    [
      "--negative-control-android-device-lab-pending-queue-closed-schema",
      /queue\/pending_queue\.json contains unexpected field[\s\S]*?queue\/pending_queue\.json ignores unexpected field/u,
      "Android device-lab pending queue closed schema gate",
    ],
    [
      "--negative-control-android-device-lab-pending-queue-empty-after-handoff",
      /queue\/pending_queue\.json pending_transactions must be empty after D2D handoff[\s\S]*?queue\/pending_queue\.json pending_transactions may remain queued after D2D handoff/u,
      "Android device-lab pending queue empty-after-handoff gate",
    ],
    [
      "--negative-control-android-device-lab-physical-device",
      /attestation\/result\.json physical_device_attestation must be true[\s\S]*?attestation\/result\.json physical_device_attestation may be false/u,
      "Android device-lab physical-device attestation",
    ],
    [
      "--negative-control-android-device-lab-private-key-ancestors",
      /private key ancestor directory[\s\S]*?private key ancestor path/u,
      "Android device-lab private key ancestor gate",
    ],
    [
      "--negative-control-android-device-lab-private-key-file-metadata-failure",
      /private key file metadata could not be read[\s\S]*?private_key_mode = None/u,
      "Android device-lab private key file metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-private-key-hardlink-metadata-failure",
      /private_key_path\.stat\(\)\.st_nlink[\s\S]*?private key hardlink metadata could not be read[\s\S]*?private_key_path\.stat\(\)\.st_nlink/u,
      "Android device-lab private key hardlink metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-private-key-missing-before-openssl",
      /private_key_mode is None[\s\S]*?private key must point to an existing file[\s\S]*?pass/u,
      "Android device-lab private key missing-before-OpenSSL gate",
    ],
    [
      "--negative-control-android-device-lab-private-key-path-before-openssl",
      /def _sign_ed25519\(private_key_path: Path, payload: bytes, errors: list\[str\]\)[\s\S]*?_secret_key_path_error\(private_key_path, "private key"\)[\s\S]*?openssl = device_lab\._require_openssl\(errors\)/u,
      "Android device-lab private key path-before-OpenSSL gate",
    ],
    [
      "--negative-control-android-device-lab-private-key-regular-file-before-openssl",
      /not stat\.S_ISREG\(private_key_mode\)[\s\S]*?private key must be a regular file[\s\S]*?False and not stat\.S_ISREG\(private_key_mode\)/u,
      "Android device-lab private key regular-file-before-OpenSSL gate",
    ],
    [
      "--negative-control-android-device-lab-private-public-pair-preserves-key-path-errors",
      /verify_errors == \["signed evidence artifact signature verification failed"\][\s\S]*?errors\.extend\(verify_errors\)[\s\S]*?if verify_errors:[\s\S]*?private key did not produce a signature accepted by the signer public key/u,
      "Android device-lab private/public pair key-path error preservation gate",
    ],
    [
      "--negative-control-android-device-lab-production-claim-binding",
      /SIGNED_EVIDENCE_SLOT_TRUE_FIELDS: tuple\[str, \.\.\.\][\s\S]*?SIGNED_EVIDENCE_SLOT_OPTIONAL_TRUE_FIELDS: tuple\[str, \.\.\.\]/u,
      "Android device-lab signed production-claim binding",
    ],
    [
      "--negative-control-android-device-lab-public-key-file-metadata-failure",
      /\{label\} file metadata could not be read[\s\S]*?public_key_mode = None/u,
      "Android device-lab public key file metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-public-key-hardlink-metadata-failure",
      /public_key_path\.stat\(\)\.st_nlink[\s\S]*?\{label\} hardlink metadata could not be read[\s\S]*?public_key_path\.stat\(\)\.st_nlink/u,
      "Android device-lab public key hardlink metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-public-key-missing-before-openssl",
      /public_key_mode is None[\s\S]*?\{label\} must point to an existing public key file[\s\S]*?pass/u,
      "Android device-lab public key missing-before-OpenSSL gate",
    ],
    [
      "--negative-control-android-device-lab-public-key-openssl-invalid-key",
      /subprocess\.CalledProcessError:[\s\S]*?\{label\} must be a valid OpenSSL public key[\s\S]*?\{label\} OpenSSL public key command could not be run/u,
      "Android device-lab public key OpenSSL invalid-key gate",
    ],
    [
      "--negative-control-android-device-lab-public-key-openssl-spawn-failure",
      /OpenSSL public key command could not be run[\s\S]*?OpenSSL public key command spawn failures ignored/u,
      "Android device-lab public key OpenSSL spawn-failure gate",
    ],
    [
      "--negative-control-android-device-lab-public-key-path-before-openssl",
      /_validate_public_key_path_shape\(public_key_path, errors=errors, label=label\)[\s\S]*?openssl = _require_openssl\(errors\)[\s\S]*?openssl = _require_openssl\(errors\)[\s\S]*?_validate_public_key_path_shape/u,
      "Android device-lab public key path-before-OpenSSL gate",
    ],
    [
      "--negative-control-android-device-lab-public-key-regular-file-before-openssl",
      /not stat\.S_ISREG\(public_key_mode\)[\s\S]*?\{label\} must be a regular file[\s\S]*?False and not stat\.S_ISREG\(public_key_mode\)/u,
      "Android device-lab public key regular-file-before-OpenSSL gate",
    ],
    [
      "--negative-control-android-attestation-report-challenge-canonical",
      /if any\(ch not in "0123456789abcdef" for ch in value\):[\s\S]*?if False:/u,
      "Android attestation report canonical challenge gate",
    ],
    [
      "--negative-control-android-attestation-report-chain-path-canonical",
      /elif raw != raw\.strip\(\) or any\(ch\.isspace\(\) for ch in raw\):[\s\S]*?elif False:/u,
      "Android attestation report canonical chain path gate",
    ],
    [
      "--negative-control-android-attestation-report-chain-source-path-aliases",
      /if "\\\\\\\\" in path_text:[\s\S]*?errors\.append\(f"\{label\} path must be canonical"\)[\s\S]*?""/u,
      "Android attestation report chain source path alias gate",
    ],
    [
      "--negative-control-android-attestation-report-harness-source-path-aliases",
      /result = device_lab\._load_json\(path, "attestation harness result", errors\)[\s\S]*?result = json\.loads\(path\.read_text\(encoding="utf-8"\)\)/u,
      "Android attestation report harness-result source path alias gate",
    ],
    [
      "--negative-control-android-attestation-report-slot-id-canonical",
      /_reject_whitespace\(value, label, errors\)[\s\S]*?candidate = PurePosixPath\(value\)[\s\S]*?candidate = PurePosixPath\(value\.strip\(\)\)/u,
      "Android attestation report slot id canonical gate",
    ],
    [
      "--negative-control-android-attestation-report-identity-canonical",
      /_reject_whitespace\(value, label, errors\)[\s\S]*?if device_lab\.SECRET_RE\.search\(value\):[\s\S]*?if False:[\s\S]*?if device_lab\.SECRET_RE\.search\(value\):/u,
      "Android attestation report identity string canonical gate",
    ],
    [
      "--negative-control-android-attestation-report-strongbox-level-canonical",
      /if value not in device_lab\.STRONGBOX_LEVELS:[\s\S]*?if value\.strip\(\)\.upper\(\) not in device_lab\.STRONGBOX_LEVELS:/u,
      "Android attestation report StrongBox level canonical gate",
    ],
    [
      "--negative-control-android-attestation-report-chain-length-binding",
      /elif chain_length != certificate_count:[\s\S]*?elif False:/u,
      "Android attestation report chain-length binding gate",
    ],
    [
      "--negative-control-android-device-lab-zero-sha256-placeholders",
      /== "0" \* 64[\s\S]*?__disabled_zero_sha256_placeholder_gate__/u,
      "Android device-lab zero SHA-256 placeholder evidence",
    ],
    [
      "--negative-control-android-device-lab-source-zero-sha256-placeholders",
      /kagemusha_android_device_lab_slot\.py[\s\S]*?kagemusha_pull_android_device_lab_raw_slot\.py[\s\S]*?__disabled_zero_sha256_placeholder_gate__/u,
      "Android device-lab source zero SHA-256 placeholder evidence",
    ],
    [
      "--negative-control-android-device-lab-apk-code-path-digest-exactness",
      /KagemushaDeviceLabArtifactExportTest\.java[\s\S]*?context\.getPackageCodePath\(\)[\s\S]*?context\.getPackageName\(\)/u,
      "Android device-lab APK code-path digest exactness",
    ],
    [
      "--negative-control-android-device-lab-release-apk-binding",
      /slot\.json native_bridge_abi_version must be[\s\S]*?slot\.json native_bridge_abi_version may be/u,
      "Android device-lab release APK and native ABI binding",
    ],
    [
      "--negative-control-android-device-lab-signed-harness-result",
      /attestation\/harness-result\.json challenge_hex digest must match slot\.json attestation_challenge_sha256[\s\S]*?attestation\/harness-result\.json challenge_hex digest may differ from slot\.json attestation_challenge_sha256/u,
      "Android device-lab signed harness-result contract",
    ],
    [
      "--negative-control-android-device-lab-signed-evidence-path-root",
      /slot\.json signed_evidence_artifact_path must stay under evidence\/[\s\S]*?slot\.json signed_evidence_artifact_path may point outside evidence\//u,
      "Android device-lab signed evidence artifact path root",
    ],
    [
      "--negative-control-android-device-lab-signed-evidence-path-canonical",
      /slot\.json signed_evidence_artifact_path must be[\s\S]*?slot\.json signed_evidence_artifact_path may be/u,
      "Android device-lab signed evidence canonical artifact path",
    ],
    [
      "--negative-control-android-device-lab-signed-device-identity-binding",
      /slot\.json device_family must match device_model\/device_codename[\s\S]*?slot\.json device_family may differ from device_model\/device_codename/u,
      "Android device-lab signed device identity binding",
    ],
    [
      "--negative-control-android-device-lab-signed-artifact-schema",
      /signed evidence artifact digest mismatch for[\s\S]*?signed evidence artifact accepts digest drift for/u,
      "Android device-lab signed evidence artifact schema",
    ],
    [
      "--negative-control-android-device-lab-signed-evidence-artifact-digest-preflight",
      /_validate_signed_evidence_artifact_for_digest[\s\S]*?assert artifact_path is not None and artifact_stat is not None[\s\S]*?artifact_path = slot_path \/ relative/u,
      "Android device-lab signed evidence artifact digest preflight gate",
    ],
    [
      "--negative-control-android-device-lab-signed-evidence-artifact-size-limit",
      /open_stat\.st_size > max_bytes[\s\S]*?False[\s\S]*?size > max_bytes[\s\S]*?False/u,
      "Android device-lab signed evidence artifact digest size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-signed-evidence-artifact-is-file-preflight",
      /_metadata_artifact_bytes_and_sha256[\s\S]*?slot\.json signed_evidence_artifact_path must point to an existing file[\s\S]*?artifact_path\.is_file\(\)/u,
      "Android device-lab signed evidence artifact is_file preflight gate",
    ],
    [
      "--negative-control-android-device-lab-signed-evidence-artifact-read-failure",
      /signed evidence artifact digest references artifact that could not be read[\s\S]*?return None, \[\]/u,
      "Android device-lab signed evidence artifact digest read-failure gate",
    ],
    [
      "--negative-control-android-device-lab-signed-evidence-artifact-open-path-binding",
      /signed_evidence_expected_identity = \([\s\S]*?expected_stat\.st_dev[\s\S]*?expected_stat\.st_ino[\s\S]*?signed_evidence_expected_identity = \([\s\S]*?open_stat\.st_dev[\s\S]*?open_stat\.st_ino/u,
      "Android device-lab signed evidence artifact open-path binding gate",
    ],
    [
      "--negative-control-android-device-lab-signature-verify",
      /signed evidence artifact signature verification failed[\s\S]*?signed evidence artifact signature verification skipped/u,
      "Android device-lab signed evidence signature verification",
    ],
    [
      "--negative-control-android-device-lab-signature-verify-staging-write-failure",
      /handle\.flush\(\)[\s\S]*?os\.fsync\(handle\.fileno\(\)\)[\s\S]*?with path\.open\("xb"\) as handle:[\s\S]*?handle\.write\(payload\)/u,
      "Android device-lab signature verification staging write-failure gate",
    ],
    [
      "--negative-control-android-device-lab-signature-verify-tempdir-failure",
      /signature verification temporary directory could not be created[\s\S]*?signature verification temporary directory failures ignored/u,
      "Android device-lab signature verification tempdir failure gate",
    ],
    [
      "--negative-control-android-device-lab-signature-verify-spawn-failure",
      /signature verification command could not be run[\s\S]*?signature verification command spawn failures ignored/u,
      "Android device-lab signature verification spawn-failure gate",
    ],
    [
      "--negative-control-android-device-lab-signed-evidence-canonical-payload-strict-json",
      /signed evidence artifact signature payload is not strict JSON[\s\S]*?signed evidence artifact signature payload allows non-strict JSON/u,
      "Android device-lab signed evidence canonical payload strict JSON gate",
    ],
    [
      "--negative-control-android-device-lab-signer-key-files",
      /private key must not be a symlink[\s\S]*?private key may be a symlink/u,
      "Android device-lab signer key-file alias gate",
    ],
    [
      "--negative-control-android-device-lab-signer-key-ancestors",
      /test_trusted_signer_public_key_rejects_symlinked_ancestor_without_path_leak[\s\S]*?test_trusted_signer_public_key_allows_symlinked_ancestor_without_path_leak/u,
      "Android device-lab trusted signer key ancestor gate",
    ],
    [
      "--negative-control-android-device-lab-signature-verify-key-path-before-openssl",
      /_validate_public_key_path_shape\(public_key_path, errors=errors, label=label\)[\s\S]*?openssl = _require_openssl\(errors\)[\s\S]*?openssl = _require_openssl\(errors\)[\s\S]*?_validate_public_key_path_shape\(public_key_path, errors=errors, label=label\)/u,
      "Android device-lab signature verifier key path-before-OpenSSL gate",
    ],
    [
      "--negative-control-android-device-lab-signer-key-secret-paths",
      /_secret_key_path_error\(private_key_path, "private key"\),[\s\S]*?_secret_key_path_error\(public_key_path, "signer public key"\),[\s\S]*?""/u,
      "Android device-lab signer key secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper",
      /device_lab\._canonical_signed_evidence_payload[\s\S]*?json\.dumps/u,
      "Android device-lab signed evidence helper",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-canonical-payload-strict-json",
      /signed evidence payload is not strict JSON[\s\S]*?signed evidence payload allows non-strict JSON/u,
      "Android device-lab signing helper canonical payload strict JSON gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-signature-read-failure",
      /signature output could not be read[\s\S]*?return b""\.join\(chunks\)[\s\S]*?except OSError:[\s\S]*?return None/u,
      "Android device-lab signed evidence helper signature read-failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-signature-open-path-binding",
      /signature_output_expected_identity = \([\s\S]*?expected_stat\.st_dev[\s\S]*?expected_stat\.st_ino[\s\S]*?signature_output_expected_identity = \([\s\S]*?open_stat\.st_dev[\s\S]*?open_stat\.st_ino/u,
      "Android device-lab signed evidence helper signature open-path binding gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-signature-output-hardlink",
      /open_stat\.st_nlink > 1[\s\S]*?signature output could not be read[\s\S]*?""/u,
      "Android device-lab signed evidence helper signature output hardlink gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-signature-output-read-limit",
      /read_limit = device_lab\.ED25519_SIGNATURE_BYTES \+ 1[\s\S]*?read_limit = 1024 \* 1024/u,
      "Android device-lab signed evidence helper signature output read-limit gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-signature-shape",
      /len\(signature\) != device_lab\.ED25519_SIGNATURE_BYTES[\s\S]*?False/u,
      "Android device-lab signed evidence helper signature shape gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-signature-staging-write-failure",
      /_write_staged_bytes[\s\S]*?signature payload could not be staged[\s\S]*?payload_path\.write_bytes\(payload\)/u,
      "Android device-lab signed evidence helper signature staging write-failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-signature-tempdir-failure",
      /signature temporary directory could not be created[\s\S]*?signature temporary directory failures ignored/u,
      "Android device-lab signed evidence helper signature tempdir failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-signature-spawn-failure",
      /signature command could not be run[\s\S]*?signature command spawn failures ignored/u,
      "Android device-lab signed evidence helper signature spawn-failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-signature-invalid-private-key",
      /private key must be a valid OpenSSL Ed25519 private key[\s\S]*?signature command could not be run/u,
      "Android device-lab signed evidence helper signature invalid-private-key gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-cli-secret-paths",
      /signed evidence output path must not contain secret-looking material[\s\S]*?signed evidence output path may contain secret-looking material/u,
      "Android device-lab signing helper CLI secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-dangling-output-alias",
      /if stat\.S_ISLNK\(mode\):[\s\S]*?if False and stat\.S_ISLNK\(mode\):/u,
      "Android device-lab signed evidence helper dangling output alias gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-direct-manifest-shape",
      /errors = _validate_slot_for_manifest_rewrite\(slot_path\)[\s\S]*?errors = \[\]/u,
      "Android device-lab signed evidence helper direct manifest shape gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-slot-listing-failure",
      /slot_files = device_lab\._slot_files\(slot_path, errors\)[\s\S]*?slot_files = device_lab\._slot_files\(slot_path\)/u,
      "Android device-lab signing helper slot listing failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-slot-metadata-failure",
      /slot directory metadata could not be read[\s\S]*?slot_mode = None/u,
      "Android device-lab signed evidence helper slot metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-slot-parent-metadata-failure",
      /slot parent directory metadata could not be read[\s\S]*?parent_mode = None/u,
      "Android device-lab signed evidence helper slot parent metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-slot-artifact-digest-preflight",
      /_validate_slot_artifact_for_digest[\s\S]*?assert artifact_path is not None and artifact_stat is not None[\s\S]*?artifact_path = slot_path \/ relative/u,
      "Android device-lab signed evidence helper slot artifact digest preflight gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-slot-artifact-size-limit",
      /open_stat\.st_size > artifact_max_bytes[\s\S]*?False[\s\S]*?size > artifact_max_bytes[\s\S]*?False/u,
      "Android device-lab signed evidence helper slot artifact size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-slot-artifact-hardlink-metadata-failure",
      /artifact_path\.stat\(\)\.st_nlink[\s\S]*?slot artifact \{display\} hardlink metadata could not be read[\s\S]*?artifact_path\.stat\(\)\.st_nlink/u,
      "Android device-lab signed evidence helper slot artifact hardlink metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-slot-artifact-file-metadata-failure",
      /slot artifact \{display\} file metadata could not be read[\s\S]*?return artifact_path, artifact_stat, \[\]/u,
      "Android device-lab signed evidence helper slot artifact file metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-slot-artifact-read-failure",
      /slot artifact \{display\} could not be read[\s\S]*?return None, \[\]/u,
      "Android device-lab signed evidence helper slot artifact read-failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-slot-artifact-open-path-binding",
      /signer_expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?signer_expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Android device-lab signed evidence helper slot artifact open-path binding gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-direct-manifest-slot-secret-paths",
      /def _validate_slot_for_manifest_rewrite\(slot_path: Path\)[\s\S]*?path_errors = _validate_slot_path_boundary\(slot_path\)[\s\S]*?errors: list\[str\] = \[\]/u,
      "Android device-lab signed evidence helper direct manifest slot secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-direct-output-secret-paths",
      /device_lab\.SECRET_RE\.search\(path_text\)[\s\S]*?\{label\} must not contain secret-looking material[\s\S]*?""/u,
      "Android device-lab signed evidence helper direct output secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-direct-slot-path-aliases",
      /path_errors = device_lab\._slot_path_boundary_errors\(slot_path\)[\s\S]*?return path_errors[\s\S]*?""/u,
      "Android device-lab signed evidence helper direct metadata slot path-alias gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-direct-slot-secret-paths",
      /path_errors = device_lab\._slot_path_boundary_errors\(slot_path\)[\s\S]*?return path_errors[\s\S]*?""/u,
      "Android device-lab signed evidence helper direct metadata slot secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-json-output-path-aliases",
      /if "\.\." in path\.parts:[\s\S]*?\{label\} must be canonical[\s\S]*?""/u,
      "Android device-lab signed evidence helper JSON output path-alias gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-json-write-failure",
      /os\.replace\([\s\S]*?src_dir_fd=parent_fd[\s\S]*?dst_dir_fd=parent_fd[\s\S]*?path\.write_text\(text, encoding="utf-8"\)/u,
      "Android device-lab signed evidence helper JSON write-failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-manifest-secret-paths",
      /slot_files = device_lab\._slot_files\(slot_path, errors\)[\s\S]*?device_lab\.SECRET_RE\.search\(relative\)[\s\S]*?slot artifacts must not contain secret-looking material[\s\S]*?slot_files = device_lab\._slot_files\(slot_path, errors\)/u,
      "Android device-lab signed evidence helper manifest secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-manifest-size-limit",
      /max_bytes=device_lab\.MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES,[\s\S]*?max_bytes=None,/u,
      "Android device-lab signed evidence helper manifest size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-manifest-write",
      /slot_path \/ "sha256sum\.txt"[\s\S]*?slot_path \/ "sha256sum\.unchecked"/u,
      "Android device-lab signed evidence helper manifest write gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-metadata-preflight",
      /_preflight_slot_metadata_reads\(slot_path\)[\s\S]*?return None, errors[\s\S]*?errors = \[\]/u,
      "Android device-lab signed evidence helper metadata preflight gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-artifact-digests-preflight",
      /preflight_errors = _preflight_slot_metadata_reads\(slot_path\)[\s\S]*?errors\.extend\(preflight_errors\)[\s\S]*?return None[\s\S]*?""/u,
      "Android device-lab signed evidence helper artifact digest preflight gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-write",
      /_write_json\(output_path, evidence, "signed evidence output path"\)[\s\S]*?_write_json\(output_path, evidence, "unchecked signed evidence output path"\)/u,
      "Android device-lab signed evidence helper output write gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-strict-json-write",
      /json\.dumps\(payload, indent=2, sort_keys=True, allow_nan=False\)[\s\S]*?json\.dumps\(payload, indent=2, sort_keys=True\)/u,
      "Android device-lab signed evidence helper strict JSON write gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-size-limit",
      /len\(text\.encode\("utf-8"\)\) > device_lab\.MAX_ANDROID_DEVICE_LAB_JSON_BYTES[\s\S]*?False and len\(text\.encode\("utf-8"\)\) > device_lab\.MAX_ANDROID_DEVICE_LAB_JSON_BYTES/u,
      "Android device-lab signed evidence helper output size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-ancestor",
      /validate_no_symlink_ancestors[\s\S]*?\{label\} ancestor directory[\s\S]*?if not parent_exists:/u,
      "Android device-lab signed evidence helper output ancestor gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-parent-is-dir-preflight",
      /not stat\.S_ISDIR\(parent_mode\)[\s\S]*?not parent\.is_dir\(\)/u,
      "Android device-lab signed evidence helper output parent is_dir preflight gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-parent-metadata-failure",
      /\{label\} parent directory metadata could not be read[\s\S]*?return False, \[\]/u,
      "Android device-lab signed evidence helper output parent metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-parent-create-failure",
      /parent\.mkdir\(mode=0o700, parents=True, exist_ok=True\)[\s\S]*?\{label\} parent directory could not be created[\s\S]*?parent\.mkdir\(mode=0o700, parents=True, exist_ok=True\)/u,
      "Android device-lab signed evidence helper output parent-create failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-post-create-parent-preflight",
      /_validate_json_output_parent[\s\S]*?\{label\} parent must be a directory[\s\S]*?""/u,
      "Android device-lab signed evidence helper output post-create parent preflight gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-parent-sync-identity",
      /expected_identity=parent_identity[\s\S]*?expected_identity=None/u,
      "Android device-lab signed evidence helper output parent sync identity gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-published-cleanup-identity",
      /_file_identity\(file_stat\) != expected_identity[\s\S]*?False/u,
      "Android device-lab signed evidence helper published cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-resolve-failure",
      /signed evidence output path could not be resolved[\s\S]*?""/u,
      "Android device-lab signed evidence helper output resolve-failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-file-metadata-failure",
      /path\.lstat\(\)\.st_mode[\s\S]*?\{label\} file metadata could not be read[\s\S]*?except FileNotFoundError/u,
      "Android device-lab signed evidence helper output file metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-hardlink-metadata-failure",
      /link_count = path\.stat\(\)\.st_nlink[\s\S]*?\{label\} hardlink metadata could not be read[\s\S]*?link_count = path\.stat\(\)\.st_nlink/u,
      "Android device-lab signed evidence helper output hardlink metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-digest-preflight",
      /errors = _validate_existing_json_output_path\(path, label\)[\s\S]*?return None, errors[\s\S]*?""/u,
      "Android device-lab signed evidence helper output digest preflight gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-digest-parent-missing",
      /missing_error=f"\{label\} parent directory is missing"[\s\S]*?missing_error=None/u,
      "Android device-lab signed evidence helper output digest parent-missing gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-digest-leaf-missing",
      /except FileNotFoundError:[\s\S]*?\{label\} must exist before digest[\s\S]*?return \[\]/u,
      "Android device-lab signed evidence helper output digest leaf-missing gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-digest-file-metadata-failure",
      /path\.lstat\(\)\.st_mode[\s\S]*?\{label\} file metadata could not be read[\s\S]*?except FileNotFoundError/u,
      "Android device-lab signed evidence helper output digest file metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-digest-hardlink-metadata-failure",
      /link_count = path\.stat\(\)\.st_nlink[\s\S]*?\{label\} hardlink metadata could not be read[\s\S]*?link_count = path\.stat\(\)\.st_nlink/u,
      "Android device-lab signed evidence helper output digest hardlink metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-digest-size-limit",
      /open_stat\.st_size > byte_limit[\s\S]*?False/u,
      "Android device-lab signed evidence helper output digest size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-digest-read-failure",
      /\{label\} could not be read[\s\S]*?return None, \[\]/u,
      "Android device-lab signed evidence helper output digest read-failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-output-digest-open-path-binding",
      /signer_output_expected_identity = \([\s\S]*?expected_stat\.st_dev[\s\S]*?expected_stat\.st_ino[\s\S]*?signer_output_expected_identity = \([\s\S]*?open_stat\.st_dev[\s\S]*?open_stat\.st_ino/u,
      "Android device-lab signed evidence helper output digest open-path binding gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-post-write-preflight",
      /_validate_existing_json_output_path\(path, label\)[\s\S]*?return errors[\s\S]*?if stat\.S_ISLNK\(expected_stat\.st_mode\)/u,
      "Android device-lab signed evidence helper post-write preflight gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-readback-verification",
      /readback_text != text[\s\S]*?False/u,
      "Android device-lab signed evidence helper readback gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-readback-failure",
      /read_errors == \[f"\{label\} could not be read"\][\s\S]*?if False:/u,
      "Android device-lab signed evidence helper readback failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-temp-cleanup-failure",
      /return \[f"\{label\} temporary file could not be removed"\][\s\S]*?return \[\]/u,
      "Android device-lab signed evidence helper temp cleanup failure gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-temp-cleanup-identity",
      /_file_identity\(temp_stat\) != expected_identity[\s\S]*?False/u,
      "Android device-lab signed evidence helper temp cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-text-size-limit",
      /len\(text\.encode\("utf-8"\)\) > byte_limit[\s\S]*?False and len\(text\.encode\("utf-8"\)\) > byte_limit/u,
      "Android device-lab signed evidence helper text size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-text-write-failure",
      /os\.fsync\(handle\.fileno\(\)\)[\s\S]*?handle\.fileno\(\)/u,
      "Android device-lab signed evidence helper text write-failure gate",
    ],
    [
      "--negative-control-android-device-lab-regular-file-artifacts",
      /sha256sum\.txt references non-regular artifact[\s\S]*?sha256sum\.txt accepts non-regular artifact/u,
      "Android device-lab regular-file artifact gate",
    ],
    [
      "--negative-control-android-device-lab-required-artifacts",
      /signed evidence artifact required slot artifact is missing[\s\S]*?signed evidence artifact required slot artifact may be omitted/u,
      "Android device-lab required artifact gate",
    ],
    [
      "--negative-control-android-device-lab-required-artifact-is-file-preflight",
      /_slot_artifact_lstat_mode[\s\S]*?stat\.S_ISLNK\(mode\) or not stat\.S_ISREG\(mode\)[\s\S]*?artifact_path\.is_file\(\)/u,
      "Android device-lab required artifact is_file preflight gate",
    ],
    [
      "--negative-control-android-device-lab-required-status-is-file-preflight",
      /_should_read_optional_text_artifact[\s\S]*?telemetry\/status\.ndjson[\s\S]*?\(slot_path \/ "telemetry" \/ "status\.ndjson"\)\.is_file\(\)/u,
      "Android device-lab required status is_file preflight gate",
    ],
    [
      "--negative-control-android-device-lab-required-runtime-log-is-file-preflight",
      /_should_read_optional_text_artifact[\s\S]*?logs\/runtime\.log[\s\S]*?\(slot_path \/ "logs" \/ "runtime\.log"\)\.is_file\(\)/u,
      "Android device-lab required runtime log is_file preflight gate",
    ],
    [
      "--negative-control-android-device-lab-required-artifact-shape",
      /artifact_size == 0[\s\S]*?False/u,
      "Android device-lab required artifact shape gate",
    ],
    [
      "--negative-control-android-device-lab-required-artifact-metadata-failure",
      /artifact_path\.stat\(\)\.st_size[\s\S]*?required slot artifact metadata could not be read \{relative\}[\s\S]*?artifact_path\.stat\(\)\.st_size/u,
      "Android device-lab required artifact metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-required-artifact-content",
      /logs\/runtime\.log must contain Kagemusha device-lab completion marker[\s\S]*?logs\/runtime\.log may omit Kagemusha device-lab completion marker/u,
      "Android device-lab required artifact content gate",
    ],
    [
      "--negative-control-android-device-lab-required-text-artifact-read-preflight",
      /text, read_errors = _metadata_artifact_text[\s\S]*?telemetry\/status\.ndjson could not be read[\s\S]*?text = \(slot_path \/ "telemetry" \/ "status\.ndjson"\)\.read_text/u,
      "Android device-lab required text artifact read preflight gate",
    ],
    [
      "--negative-control-android-device-lab-relative-ancestor-is-symlink-preflight",
      /stat\.S_ISLNK\(current_mode\)[\s\S]*?current\.is_symlink\(\)/u,
      "Android device-lab relative ancestor is_symlink preflight gate",
    ],
    [
      "--negative-control-android-device-lab-scan-slot-expected-dir-is-dir-preflight",
      /stat\.S_ISLNK\(dir_mode\) or not stat\.S_ISDIR\(dir_mode\)[\s\S]*?stat\.S_ISLNK\(dir_mode\) or not dir_path\.is_dir\(\)/u,
      "Android device-lab scan_slot expected directory is_dir preflight gate",
    ],
    [
      "--negative-control-android-device-lab-scan-slot-artifact-count-is-file-preflight",
      /stat\.S_ISREG\(entry_mode\)[\s\S]*?entry\.is_file\(\)/u,
      "Android device-lab scan_slot artifact count is_file preflight gate",
    ],
    [
      "--negative-control-android-device-lab-scan-slot-sha-is-file-preflight",
      /return stat\.S_ISREG\(mode\)[\s\S]*?return path\.is_file\(\)/u,
      "Android device-lab scan_slot sha256sum is_file preflight gate",
    ],
    [
      "--negative-control-android-device-lab-secret-redaction",
      /\{_display_path\(relative\)\}[\s\S]*?\{relative\}/u,
      "Android device-lab secret-looking path redaction",
    ],
    [
      "--negative-control-android-device-lab-root-direct-secret-paths",
      /SECRET_RE\.search\(root_text\)[\s\S]*?device-lab root path must not contain secret-looking material[\s\S]*?""/u,
      "Android device-lab direct root secret-path gate",
    ],
    [
      "--negative-control-android-device-lab-root-direct-control-paths",
      /_contains_control_character\(root_text\)[\s\S]*?device-lab root path must not contain control characters[\s\S]*?""/u,
      "Android device-lab direct root control-path gate",
    ],
    [
      "--negative-control-android-device-lab-root-direct-path-aliases",
      /device-lab root path must not contain backslashes[\s\S]*?""/u,
      "Android device-lab direct root path-alias gate",
    ],
    [
      "--negative-control-android-device-lab-root-metadata-failure",
      /root\.lstat\(\)\.st_mode[\s\S]*?device-lab root metadata could not be read[\s\S]*?except FileNotFoundError/u,
      "Android device-lab direct root metadata failure gate",
    ],
    [
      "--negative-control-android-device-lab-rollup-root-exists-preflight",
      /if not root_exists:[\s\S]*?"ok": False,[\s\S]*?if not root\.exists\(\):/u,
      "Android device-lab rollup root exists preflight gate",
    ],
    [
      "--negative-control-android-device-lab-root-symlink",
      /device-lab root must not be a symlink[\s\S]*?device-lab root may be a symlink/u,
      "Android device-lab root symlink gate",
    ],
    [
      "--negative-control-android-device-lab-root-ancestor-symlink",
      /device-lab root ancestor directory[\s\S]*?device-lab root ancestor path/u,
      "Android device-lab root ancestor symlink gate",
    ],
    [
      "--negative-control-android-device-lab-root-discovery-read-failure",
      /device-lab root could not be listed[\s\S]*?device-lab root listing failures ignored/u,
      "Android device-lab root discovery read-failure gate",
    ],
    [
      "--negative-control-android-device-lab-scanner-harness-canonical",
      /if level is not None and level not in STRONGBOX_LEVELS:[\s\S]*?if level is not None and level\.upper\(\) not in STRONGBOX_LEVELS:/u,
      "Android scanner harness canonical string gate",
    ],
    [
      "--negative-control-android-device-lab-root-summary-label-exactness",
      /DEVICE_LAB_ROOT_SUMMARY_LABEL = "<local-device-lab-root>"\\n[\s\S]*?DEVICE_LAB_ROOT_SUMMARY_LABEL_DISABLED = "<local-device-lab-root>"\\n[\s\S]*?ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL = "<local-device-lab-root>"\\n[\s\S]*?ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL_DISABLED = "<local-device-lab-root>"\\n/u,
      "Android device-lab root summary label exactness",
    ],
    [
      "--negative-control-android-device-lab-telemetry-suite-exactness",
      /KAGEMUSHA_TELEMETRY_SUITE = "kagemusha-device-lab"\\n[\s\S]*?KAGEMUSHA_TELEMETRY_SUITE_DISABLED = "kagemusha-device-lab"\\n/u,
      "Android device-lab telemetry suite exactness",
    ],
    [
      "--negative-control-android-device-lab-size-cap-constant-exactness",
      /MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 16 \* 1024 \* 1024\\n[\s\S]*?MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES_DISABLED = 16 \* 1024 \* 1024\\n[\s\S]*?MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES_DISABLED = 1024 \* 1024\\n/u,
      "Android device-lab size-cap constant exactness",
    ],
    [
      "--negative-control-android-device-lab-doc-install-marker-exactness",
      /docs\/source\/sdk\/android\/readiness\/android_strongbox_device_matrix\.md[\s\S]*?`:offline-wallet-lab-app:installRelease`, and\\n[\s\S]*?""/u,
      "Android device-lab documentation install marker exactness",
    ],
    [
      "--negative-control-android-device-lab-raw-command-exact",
      /must exactly match the Kagemusha Android production raw test command[\s\S]*?may contain Kagemusha Android production raw test command markers/u,
      "Android device-lab exact raw command gate",
    ],
    [
      "--negative-control-android-device-lab-raw-command-marker-specificity",
      /org\.hyperledger\.iroha\.android\.offline\.KagemushaRecursiveSpendProverTest",\\n[\s\S]*?KagemushaRecursiveSpendProverTest",\\n[\s\S]*?org\.hyperledger\.iroha\.android\.offline\.OfflineNoteTransferHandoffTest",\\n[\s\S]*?OfflineNoteTransferHandoffTest",\\n/u,
      "Android device-lab raw command marker specificity",
    ],
    [
      "--negative-control-android-device-lab-raw-command-constant-exactness",
      /KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_COMMAND = \([\s\S]*?KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS_EXPORT = \(/u,
      "Android device-lab raw command constant exactness",
    ],
    [
      "--negative-control-android-device-lab-raw-command-marker-tuple-exactness",
      /RAW_TEST_COMMAND_REQUIRED_MARKERS: tuple\[str, \.\.\.\] = \([\s\S]*?RAW_TEST_COMMAND_REQUIRED_MARKERS_DISABLED: tuple\[str, \.\.\.\] = \(/u,
      "Android device-lab raw command marker tuple exactness",
    ],
    [
      "--negative-control-android-device-matrix-attestation-result-doc-exactness",
      /verifier report it independently requires `attestation\/result\.json` to match\\n[\s\S]*?""/u,
      "Android device-matrix attestation-result doc exactness",
    ],
    [
      "--negative-control-android-device-matrix-physical-attestation-doc-exactness",
      /explicit `--physical-device-attestation` operator assertion, rejects\\n[\s\S]*?""/u,
      "Android device-matrix physical-attestation doc exactness",
    ],
    [
      "--negative-control-android-device-matrix-generated-at-utc-doc-exactness",
      /\\t  `generated_at_utc` must use canonical UTC\\n[\s\S]*?""/u,
      "Android device-matrix generated-at UTC doc exactness",
    ],
    [
      "--negative-control-android-device-matrix-signed-evidence-path-doc-exactness",
      /The signed evidence artifact path must be the canonical\\n[\s\S]*?""/u,
      "Android device-matrix signed-evidence path doc exactness",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-blank-serial",
      /if args\.serial is not None:[\s\S]*?if args\.serial:/u,
      "Android raw puller blank serial gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-overwrite",
      /slot directory already exists; refuse to overwrite raw evidence[\s\S]*?slot directory already exists; replacing raw evidence/u,
      "Android raw puller overwrite refusal gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-install-no-overwrite",
      /os\.mkdir\(final_slot\.name, 0o700, dir_fd=output_root_fd\)[\s\S]*?os\.makedirs\(final_slot, mode=0o700, exist_ok=True\)/u,
      "Android raw puller install-time overwrite refusal gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-install-top-level",
      /raw slot install source contains unexpected top-level entry[\s\S]*?raw slot install source accepts unexpected top-level entry/u,
      "Android raw puller install top-level allowlist gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-install-parent-sync",
      /raw slot directory parent could not be synced[\s\S]*?raw slot directory parent sync is optional/u,
      "Android raw puller install parent-sync gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-install-directory-identity",
      /raw slot directory changed during install[\s\S]*?raw slot directory identity drift is accepted/u,
      "Android raw puller install directory-identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-install-sync-identity",
      /expected_identity=output_root_identity[\s\S]*?expected_identity=None/u,
      "Android raw puller install sync identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-install-cleanup-identity",
      /and _file_identity\(path_stat\) == expected_identity[\s\S]*?and True/u,
      "Android raw puller install cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-install-cleanup-report",
      /return \[\*install_errors, \*cleanup_errors\][\s\S]*?return install_errors/u,
      "Android raw puller install cleanup report gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-temp-cleanup-identity",
      /_file_identity\(temp_parent_stat\) != expected_identity[\s\S]*?False/u,
      "Android raw puller temp cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-temp-cleanup-report",
      /if pull_errors or cleanup_errors:[\s\S]*?if pull_errors:/u,
      "Android raw puller temp cleanup report gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-install-rename-dir-fd",
      /src_dir_fd=stage_fd,\\n\s*dst_dir_fd=final_fd,[\s\S]*?src_dir_fd=None,\\n\s*dst_dir_fd=None,/u,
      "Android raw puller install rename dir-fd gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-install-output-root-identity",
      /expected_identity=output_root_identity[\s\S]*?expected_identity=None/u,
      "Android raw puller install output-root identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-install-cleanup-dir-fd",
      /shutil\.rmtree\(name, dir_fd=parent_fd\)[\s\S]*?shutil\.rmtree\(name\)/u,
      "Android raw puller install cleanup dir-fd gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-install-slot-entry-dir-fd",
      /os\.stat\(final_slot\.name, dir_fd=output_root_fd, follow_symlinks=False\)[\s\S]*?final_slot\.lstat\(\)/u,
      "Android raw puller install slot-entry dir-fd gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-path-aliases",
      /raw output root path must not contain backslashes[\s\S]*?raw output root path may contain backslashes/u,
      "Android raw puller path-alias gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-allowed-artifacts",
      /raw slot artifact \{relative\} is not an allowed path[\s\S]*?raw slot artifact \{relative\} may be an unreviewed debug path/u,
      "Android raw puller closed artifact set gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-directory-collision",
      /raw slot tar directory \{relative\} could not be created[\s\S]*?raw slot tar directory collisions are ignored/u,
      "Android raw puller tar directory collision gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-entry-cap",
      /entry_count \+= 1[\s\S]*?entry_count \+= 0/u,
      "Android raw puller tar entry cap",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-summary-strict-json",
      /raw pull summary output is not strict JSON[\s\S]*?raw pull summary output may contain non-finite JSON/u,
      "Android raw puller summary strict-JSON gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-summary-size-limit",
      /len\(encoded\) > device_lab\.MAX_ANDROID_DEVICE_LAB_JSON_BYTES[\s\S]*?False/u,
      "Android raw puller summary size-limit gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-summary-parent-sync",
      /raw pull summary output parent directory could not be synced[\s\S]*?raw pull summary output parent sync is optional/u,
      "Android raw puller summary parent-sync gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-summary-parent-identity",
      /expected_identity=parent_identity[\s\S]*?expected_identity=None[\s\S]*?_file_identity\(current_parent_stat\) != parent_identity[\s\S]*?False/u,
      "Android raw puller summary parent identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-summary-readback-symlink",
      /f"\{label\} must not be a symlink after writing"[\s\S]*?f"\{label\} symlink readback is accepted"/u,
      "Android raw puller summary readback symlink gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-summary-readback-hardlink",
      /f"\{label\} must not be hardlinked after writing"[\s\S]*?f"\{label\} hardlink readback is accepted"/u,
      "Android raw puller summary readback hardlink gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-summary-readback-identity",
      /f"\{label\} changed while being read back"[\s\S]*?f"\{label\} path swaps are accepted during readback"/u,
      "Android raw puller summary readback identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-summary-private-permissions",
      /kagemusha_pull_android_device_lab_raw_slot\.py[\s\S]*?f"\{label\} permissions must be 0600"[\s\S]*?f"\{label\} may be world-readable"/u,
      "Android raw puller summary private permissions",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-summary-temp-cleanup-identity",
      /_file_identity\(temp_stat\) != expected_identity[\s\S]*?False/u,
      "Android raw puller summary temp cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-published-cleanup-identity",
      /_file_identity\(file_stat\) != expected_identity[\s\S]*?False/u,
      "Android raw puller published cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-summary-digest-open-path",
      /open_identity != expected_identity[\s\S]*?False/u,
      "Android raw puller summary digest open-path gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-summary-digest-inventory",
      /raw artifact digest inventory must include every required artifact[\s\S]*?raw artifact digest inventory may omit artifacts/u,
      "Android raw puller summary digest inventory gate",
    ],
    [
      "--negative-control-android-device-lab-raw-harness-result",
      /_validate_harness_result[\s\S]*?_trust_harness_result/u,
      "Android device-lab raw harness-result contract",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-json-slot-binding",
      /def _validate_raw_json_slot_id[\s\S]*?def _normalise_raw_json_slot_id/u,
      "Android raw puller JSON slot-binding gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-d2d-offline",
      /"transport_offline"[\s\S]*?"transport_online_optional"/u,
      "Android raw puller D2D offline-offline gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-wallet-rollback",
      /"rollback_rejection_passed"[\s\S]*?"rollback_rejection_optional"/u,
      "Android raw puller wallet rollback-rejection gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-status-failure",
      /device_lab\.KAGEMUSHA_STATUS_FAILURE_VALUES[\s\S]*?set\(\)/u,
      "Android raw puller status failure gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-runtime-failure-marker",
      /device_lab\.KAGEMUSHA_RUNTIME_LOG_FAILURE_MARKERS[\s\S]*?\(\)/u,
      "Android raw puller runtime failure-marker gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-harness-challenge",
      /attestation\/harness-result\.json challenge_hex must match attestation\/challenge\.hex[\s\S]*?attestation\/harness-result\.json challenge_hex may differ from attestation\/challenge\.hex/u,
      "Android raw puller harness challenge binding gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-harness-strongbox",
      /attestation\/harness-result\.json strongbox_attestation must be true[\s\S]*?attestation\/harness-result\.json strongbox_attestation may be false/u,
      "Android raw puller harness StrongBox claim gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-harness-chain-length",
      /attestation\/harness-result\.json chain_length must match[\s\S]*?attestation\/harness-result\.json chain_length may differ from/u,
      "Android raw puller harness certificate-chain length binding gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-harness-canonical",
      /attestation\/harness-result\.json challenge_hex must be lowercase hexadecimal without whitespace[\s\S]*?attestation\/harness-result\.json challenge_hex may be normalized/u,
      "Android raw puller harness canonical string gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-challenge-file-canonical",
      /challenge_text\.count[\s\S]*?False/u,
      "Android raw puller challenge file canonical gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-latest-slot-canonical",
      /latest_text != f"\{slot_id\}[\s\S]*?latest_text\.strip\(\) != slot_id/u,
      "Android raw puller latest-slot canonical binding gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-latest-query-canonical",
      /latest_text\.count[\s\S]*?False/u,
      "Android raw puller latest-slot query canonical gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-latest-write-parent-identity",
      /expected_identity=root_identity[\s\S]*?expected_identity=None[\s\S]*?_file_identity\(current_root_stat\) != root_identity[\s\S]*?False/u,
      "Android raw puller latest-slot writer parent identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-latest-write-readback-symlink",
      /f"\{label\} must not be a symlink after writing"[\s\S]*?f"\{label\} symlink readback is accepted"/u,
      "Android raw puller latest-slot writer symlink readback gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-latest-write-readback-hardlink",
      /f"\{label\} must not be hardlinked after writing"[\s\S]*?f"\{label\} hardlink readback is accepted"/u,
      "Android raw puller latest-slot writer hardlink readback gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-latest-write-readback-identity",
      /f"\{label\} changed while being read back"[\s\S]*?f"\{label\} path swaps are accepted during readback"/u,
      "Android raw puller latest-slot writer identity readback gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-latest-write-private-permissions",
      /kagemusha_pull_android_device_lab_raw_slot\.py[\s\S]*?f"\{label\} permissions must be 0600"[\s\S]*?f"\{label\} may be world-readable"/u,
      "Android raw puller latest-slot private permissions",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-latest-write-temp-cleanup-identity",
      /_file_identity\(temp_stat\) != expected_identity[\s\S]*?False/u,
      "Android raw puller latest-slot writer temp cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-result-slot-required",
      /result\.get\("slot"\) != slot_id[\s\S]*?result\.get\("slot"\) not in \(None, slot_id\)/u,
      "Android raw puller attestation result slot-required gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-result-chain-digest-required",
      /RAW_RESULT_CHAIN_DIGEST_FIELD = "attestation_certificate_chain_sha256"[\s\S]*?RAW_RESULT_CHAIN_DIGEST_FIELD = "attestation_certificate_chain_digest_optional"/u,
      "Android raw puller attestation result chain digest-required gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-result-challenge-digest-required",
      /RAW_RESULT_CHALLENGE_DIGEST_FIELD = "attestation_challenge_sha256"[\s\S]*?RAW_RESULT_CHALLENGE_DIGEST_FIELD = "attestation_challenge_digest_optional"/u,
      "Android raw puller attestation result challenge digest-required gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-result-closed-schema",
      /attestation\/result\.json contains unexpected field[\s\S]*?attestation\/result\.json may contain debug fields/u,
      "Android raw puller attestation result closed-schema gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-result-identity-strings",
      /def _validate_raw_result_string[\s\S]*?_normalise_raw_result_string/u,
      "Android raw puller attestation result identity string gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-result-sdk-digests",
      /for field in RAW_RESULT_SHA256_FIELDS:[\s\S]*?for field in \(\):/u,
      "Android raw puller attestation result SDK digest gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-result-strongbox-levels",
      /for field in RAW_RESULT_STRONGBOX_FIELDS:[\s\S]*?for field in \(\):/u,
      "Android raw puller attestation result StrongBox-level gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-private-permissions",
      /os\.fchmod\(dir_fd, 0o700\)[\s\S]*?os\.fstat\(dir_fd\)[\s\S]*?os\.fchmod\(output_fd, 0o600\)[\s\S]*?os\.fstat\(output_fd\)/u,
      "Android raw puller private extracted-artifact permissions",
    ],
    [
      "--negative-control-android-device-lab-attestation-report-writer-physical-device",
      /physical device attestation must be explicitly asserted with[\s\S]*?physical device attestation is optional for local reports/u,
      "Android attestation report writer physical-device assertion gate",
    ],
    [
      "--negative-control-android-device-lab-attestation-report-writer-parent-sync-identity",
      /expected_identity=parent_identity[\s\S]*?expected_identity=None/u,
      "Android attestation report writer parent sync identity gate",
    ],
    [
      "--negative-control-android-device-lab-attestation-report-writer-published-cleanup-identity",
      /device_lab\._file_identity\(file_stat\) != expected_identity[\s\S]*?False/u,
      "Android attestation report writer published cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-attestation-report-writer-temp-cleanup-failure",
      /return \[f"\{label\} temporary file could not be removed"\][\s\S]*?return \[\]/u,
      "Android attestation report writer temp cleanup failure gate",
    ],
    [
      "--negative-control-android-device-lab-attestation-report-writer-temp-cleanup-identity",
      /device_lab\._file_identity\(temp_stat\) != expected_identity[\s\S]*?False/u,
      "Android attestation report writer temp cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-attestation-report-writer-private-permissions",
      /kagemusha_android_attestation_report\.py[\s\S]*?os\.fchmod\(dir_fd, 0o700\)[\s\S]*?os\.fstat\(dir_fd\)[\s\S]*?os\.fchmod\(handle\.fileno\(\), 0o600\)[\s\S]*?handle\.fileno\(\)/u,
      "Android attestation report writer private permissions",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-private-permissions",
      /kagemusha_android_device_lab_slot\.py[\s\S]*?os\.fchmod\(dir_fd, 0o700\)[\s\S]*?os\.fstat\(dir_fd\)[\s\S]*?os\.fchmod\(out_fd, 0o600\)[\s\S]*?os\.fstat\(out_fd\)[\s\S]*?sign_android_device_lab_evidence\.py[\s\S]*?os\.fchmod\(handle\.fileno\(\), 0o600\)[\s\S]*?handle\.fileno\(\)/u,
      "Android slot assembler private published-artifact permissions",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-source-identity-fallback",
      /identity_hints=identity_hints[\s\S]*?identity_hints=\{\}/u,
      "Android device-lab slot assembler source identity fallback",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-blank-source-identity",
      /errors\.append\(f"\{label\} \{key\} must be a non-empty string"\)\\n        return None'[\s\S]*?'if value == "":\\n        return None/u,
      "Android device-lab slot assembler blank source identity",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-blank-identity-override",
      /errors\.append\(f"\{key\} must be a non-empty string"\)\\n        return None'[\s\S]*?'if override == "":\\n        return None/u,
      "Android device-lab slot assembler blank identity override",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-source-identity-conflict",
      /if hints\[key\] != value:[\s\S]*?if False and hints\[key\] != value:/u,
      "Android device-lab slot assembler source identity conflict",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-override-source-identity-binding",
      /if value is not None and hint_value is not None and value != hint_value:[\s\S]*?if False and value is not None and hint_value is not None and value != hint_value:/u,
      "Android device-lab slot assembler override source identity binding",
    ],
    [
      "--negative-control-release-bundle-top-level-evidence-path",
      /if isinstance\(entry, dict\) and entry\.get\("path"\) != expected_path:[\s\S]*?if False and isinstance\(entry, dict\) and entry\.get\("path"\) != expected_path:/u,
      "Kagemusha release bundle top-level evidence path gate",
    ],
    [
      "--negative-control-release-bundle-top-level-evidence-binding",
      /entry\.get\("path"\) == expected_entry\.get\("path"\)[\s\S]*?entry\.get\("sha256"\) == expected_entry\.get\("sha256"\)[\s\S]*?entry\.get\("size_bytes"\) == expected_entry\.get\("size_bytes"\)[\s\S]*?if True:/u,
      "Kagemusha release bundle top-level evidence binding",
    ],
    [
      "--negative-control-release-bundle-abi7-fixture-manifest-digest-binding",
      /fixture_manifest_sha256[\s\S]*?fixture_manifest_sha256_disabled/u,
      "Kagemusha release bundle ABI-7 fixture manifest digest binding",
    ],
    [
      "--negative-control-release-bundle-abi7-archive-fixture-digest-binding",
      /archive_fixture_sha256[\s\S]*?archive_fixture_sha256_disabled/u,
      "Kagemusha release bundle ABI-7 archive fixture digest binding",
    ],
    [
      "--negative-control-release-bundle-abi7-fixture-digest-shape",
      /for field in \("fixture_manifest_sha256", "archive_fixture_sha256"\):[\s\S]*?for field in \("fixture_manifest_sha256",\):/u,
      "Kagemusha release bundle ABI-7 fixture digest shape",
    ],
    [
      "--negative-control-release-bundle-abi7-section-value-binding",
      /expected_abi7_values = \{[\s\S]*?"operation_count": len\(readiness\.ABI7_FIXTURE_OPERATIONS\),[\s\S]*?expected_abi7_values = \{[\s\S]*?"circuit_id": readiness\.EXPECTED_COMPACT_KEY_CIRCUIT_ID,[\s\S]*?\}/u,
      "Kagemusha release bundle ABI-7 section value binding",
    ],
    [
      "--negative-control-release-bundle-abi7-section-shape",
      /if isinstance\(expected, str\) and \(not isinstance\(value, str\) or not value\):[\s\S]*?if False and isinstance\(expected, str\) and \(not isinstance\(value, str\) or not value\):[\s\S]*?if isinstance\(value, bool\) or not isinstance\(value, int\) or value <= 0:[\s\S]*?if False and \(isinstance\(value, bool\) or not isinstance\(value, int\) or value <= 0\):/u,
      "Kagemusha release bundle ABI-7 section shape",
    ],
    [
      "--negative-control-release-bundle-abi6-section-value-binding",
      /expected_abi6_values = \{[\s\S]*?"operation_count": len\(readiness\.ABI6_OPERATION_SYMBOLS\),[\s\S]*?expected_abi6_values = \{[\s\S]*?"schema": readiness\.ABI6_MANIFEST_SCHEMA,[\s\S]*?\}/u,
      "Kagemusha release bundle ABI-6 section value binding",
    ],
    [
      "--negative-control-release-bundle-abi6-nested-value-binding",
      /if abi6\.get\("limits"\) != expected_abi6_limits:[\s\S]*?if False and abi6\.get\("limits"\) != expected_abi6_limits:[\s\S]*?if abi6\.get\("modes"\) != expected_abi6_modes:[\s\S]*?if False and abi6\.get\("modes"\) != expected_abi6_modes:/u,
      "Kagemusha release bundle ABI-6 nested value binding",
    ],
    [
      "--negative-control-release-bundle-abi6-section-shape",
      /for field in \("manifest_path", "schema"\):[\s\S]*?if False and \(not isinstance\(value, str\) or not value\):[\s\S]*?for field in \("native_bridge_abi_version", "operation_count"\):[\s\S]*?if False and \(isinstance\(value, bool\) or not isinstance\(value, int\) or value <= 0\):[\s\S]*?for field in \("limits", "modes"\):[\s\S]*?if False and not isinstance\(abi6\.get\(field\), dict\):/u,
      "Kagemusha release bundle ABI-6 section shape",
    ],
    [
      "--negative-control-release-bundle-section-evidence-binding",
      /entry\.get\("sha256"\) == expected_sha256[\s\S]*?expected_path is None or entry\.get\("path"\) == expected_path[\s\S]*?expected_size is None or entry\.get\("size_bytes"\) == expected_size[\s\S]*?if True:/u,
      "Kagemusha release bundle section evidence binding",
    ],
    [
      "--negative-control-release-bundle-compact-generator-log-artifact-binding",
      /existing_compact\.get\(field\) == expected_compact\.get\(field\)[\s\S]*?if True:/u,
      "Kagemusha release bundle compact generator-log artifact binding",
    ],
    [
      "--negative-control-release-bundle-summary-shape",
      /_check_ready_summary_shape\(summary\)[\s\S]*?blockers\.extend\(\[\]\)/u,
      "Kagemusha release bundle summary shape",
    ],
    [
      "--negative-control-release-bundle-android-slot-entry-shape",
      /if not isinstance\(entry, dict\):[\s\S]*?if False and not isinstance\(entry, dict\):/u,
      "Kagemusha release bundle Android slot entry shape",
    ],
    [
      "--negative-control-release-bundle-android-signed-evidence-entry-shape",
      /if not isinstance\(entry, dict\):[\s\S]*?if False and not isinstance\(entry, dict\):/u,
      "Kagemusha release bundle Android signed-evidence entry shape",
    ],
    [
      "--negative-control-release-bundle-android-summary-list-shape",
      /if not field_ok:[\s\S]*?kagemusha_release_summary_android_list_shape[\s\S]*?if False and not field_ok:/u,
      "Kagemusha release bundle Android summary list shape",
    ],
    [
      "--negative-control-release-bundle-android-manifest-list-shape",
      /if not field_ok:[\s\S]*?kagemusha_release_bundle_manifest_android_list_shape[\s\S]*?if False and not field_ok:/u,
      "Kagemusha release bundle Android manifest list shape",
    ],
    [
      "--negative-control-release-bundle-android-slot-errors-shape",
      /if errors != \[\]:[\s\S]*?if False and errors != \[\]:/u,
      "Kagemusha release bundle Android slot errors shape",
    ],
    [
      "--negative-control-release-bundle-android-slot-present-shape",
      /or any\(value is not True for value in present\.values\(\)\)[\s\S]*?or False/u,
      "Kagemusha release bundle Android slot present shape",
    ],
    [
      "--negative-control-release-bundle-android-slot-file-counts-shape",
      /or any\([\s\S]*?isinstance\(value, bool\)[\s\S]*?or False and any\(/u,
      "Kagemusha release bundle Android slot file-counts shape",
    ],
    [
      "--negative-control-release-bundle-android-duplicate-binding-list-shape",
      /if not isinstance\(entries, list\):[\s\S]*?if False and not isinstance\(entries, list\):/u,
      "Kagemusha release bundle Android duplicate-binding list shape",
    ],
    [
      "--negative-control-release-bundle-android-duplicate-binding-entry-shape",
      /if not isinstance\(entry, dict\):[\s\S]*?if False and not isinstance\(entry, dict\):/u,
      "Kagemusha release bundle Android duplicate-binding entry shape",
    ],
    [
      "--negative-control-release-bundle-android-duplicate-binding-entry-schema",
      /set\(entry\) - ANDROID_DUPLICATE_BINDING_ENTRY_FIELDS[\s\S]*?unexpected_fields = \[\]/u,
      "Kagemusha release bundle Android duplicate-binding entry schema",
    ],
    [
      "--negative-control-release-bundle-android-duplicate-binding-slot-binding",
      /and slot not in signed_evidence_summary[\s\S]*?and False/u,
      "Kagemusha release bundle Android duplicate-binding slot binding",
    ],
    [
      "--negative-control-release-bundle-android-duplicate-binding-value-binding",
      /if kagemusha\.get\(raw_field\) == value_sha256:[\s\S]*?if True:/u,
      "Kagemusha release bundle Android duplicate-binding value binding",
    ],
    [
      "--negative-control-release-bundle-android-duplicate-binding-value-inventory",
      /valid_value_sha256s != sorted\(set\(valid_value_sha256s\)\)[\s\S]*?False/u,
      "Kagemusha release bundle Android duplicate-binding value inventory",
    ],
    [
      "--negative-control-release-bundle-evidence-inventory-schema",
      /_check_release_bundle_evidence_inventory_shape\(evidence\)[\s\S]*?blockers\.extend\(\[\]\)/u,
      "Kagemusha release bundle evidence inventory schema",
    ],
    [
      "--negative-control-release-bundle-evidence-inventory-keysets",
      /_check_release_bundle_cross_section_shape\(bundle\)[\s\S]*?blockers\.extend\(\[\]\)/u,
      "Kagemusha release bundle evidence inventory key sets",
    ],
    [
      "--negative-control-release-bundle-section-schema",
      /_check_release_bundle_section_shapes\(bundle\)[\s\S]*?blockers\.extend\(\[\]\)/u,
      "Kagemusha release bundle section schema",
    ],
    [
      "--negative-control-release-bundle-android-manifest-schema",
      /_check_release_bundle_android_section_shape\(bundle\)[\s\S]*?blockers\.extend\(\[\]\)/u,
      "Kagemusha release bundle Android manifest schema",
    ],
    [
      "--negative-control-release-bundle-manifest-shape",
      /_check_release_bundle_manifest_shape\(existing\)[\s\S]*?shape_blockers = \[\]/u,
      "Kagemusha release bundle manifest shape",
    ],
    [
      "--negative-control-release-bundle-android-summary-binding",
      /existing_android\.get\(field\) == expected_android\.get\(field\)[\s\S]*?if True:/u,
      "Kagemusha release bundle Android summary field binding",
    ],
    [
      "--negative-control-release-bundle-android-signed-evidence-summary-binding",
      /if existing_signed != expected_signed:[\s\S]*?if False and existing_signed != expected_signed:/u,
      "Kagemusha release bundle Android signed-evidence summary binding",
    ],
    [
      "--negative-control-release-bundle-android-signed-evidence-binding",
      /entry\.get\("path"\) == expected_entry\.get\("path"\)[\s\S]*?entry\.get\("sha256"\) == expected_entry\.get\("sha256"\)[\s\S]*?entry\.get\("size_bytes"\) == expected_entry\.get\("size_bytes"\)[\s\S]*?if True:/u,
      "Kagemusha release bundle Android signed-evidence entry binding",
    ],
    [
      "--negative-control-release-bundle-android-slot-artifact-binding",
      /entry\.get\("path"\) == expected_entry\.get\("path"\)[\s\S]*?entry\.get\("sha256"\) == expected_entry\.get\("sha256"\)[\s\S]*?entry\.get\("size_bytes"\) == expected_entry\.get\("size_bytes"\)[\s\S]*?if True:/u,
      "Kagemusha release bundle Android slot artifact entry binding",
    ],
    [
      "--negative-control-release-bundle-android-signed-evidence-identity",
      /device_lab\.infer_kagemusha_device_family[\s\S]*?device_lab\.accept_any_kagemusha_device_family/u,
      "Kagemusha release bundle Android signed-evidence identity binding",
    ],
    [
      "--negative-control-release-bundle-android-slot-summary-identity",
      /kagemusha_release_summary_android_slots_device_identity[\s\S]*?android_slots_device_identity_disabled/u,
      "Kagemusha release bundle Android slot summary identity binding",
    ],
    [
      "--negative-control-release-bundle-android-signed-evidence-identity-drift",
      /kagemusha_release_summary_android_signed_evidence_identity_drift[\s\S]*?android_signed_evidence_identity_drift_disabled/u,
      "Kagemusha release bundle Android signed-evidence identity drift",
    ],
    [
      "--negative-control-release-bundle-android-slot-identity-drift",
      /kagemusha_release_summary_android_slots_identity_drift[\s\S]*?android_slots_identity_drift_disabled/u,
      "Kagemusha release bundle Android slot identity drift",
    ],
    [
      "--negative-control-release-bundle-manifest-android-signed-evidence-identity-binding",
      /kagemusha_release_bundle_manifest_android_signed_evidence_identity_binding[\s\S]*?android_manifest_signed_evidence_identity_binding_disabled/u,
      "Kagemusha release bundle manifest Android signed-evidence identity binding",
    ],
    [
      "--negative-control-release-bundle-android-signer-binding",
      /or signer in trusted_signer_set[\s\S]*?or True/u,
      "Kagemusha release bundle Android signer binding",
    ],
    [
      "--negative-control-release-bundle-cli-missing-evidence-summary",
      /test_kagemusha_release_bundle_rejects_cli_missing_evidence_summary_without_path_leak[\s\S]*?test_kagemusha_release_bundle_accepts_cli_missing_evidence_summary_without_path_leak/u,
      "Kagemusha release bundle CLI missing-evidence readiness summary coverage",
    ],
    [
      "--negative-control-release-bundle-ready-summary-top-level-blockers",
      /test_kagemusha_release_bundle_rejects_ready_summary_top_level_blockers_without_leak[\s\S]*?test_kagemusha_release_bundle_accepts_ready_summary_top_level_blockers_without_leak/u,
      "Kagemusha release bundle ready-summary top-level blocker coverage",
    ],
    [
      "--negative-control-release-bundle-ready-manifest-top-level-blockers",
      /test_kagemusha_release_bundle_verify_existing_rejects_ready_manifest_top_level_blockers_without_leak[\s\S]*?test_kagemusha_release_bundle_verify_existing_accepts_ready_manifest_top_level_blockers_without_leak/u,
      "Kagemusha release bundle ready-manifest top-level blocker coverage",
    ],
    [
      "--negative-control-abi7-fixture-closed-schema",
      /abi7_fixture_manifest_unexpected_field[\s\S]*?abi7_fixture_manifest_unchecked_field/u,
      "ABI-7 fixture closed schema",
    ],
    [
      "--negative-control-abi7-fixture-nested-manifest-closed-schema",
      /abi7_fixture_manifest_generator_unexpected_field[\s\S]*?abi7_fixture_manifest_generator_unchecked_field[\s\S]*?abi7_fixture_manifest_domains_unexpected_field[\s\S]*?abi7_fixture_manifest_domains_unchecked_field/u,
      "ABI-7 fixture nested manifest closed schema",
    ],
    [
      "--negative-control-abi7-fixture-nested-object-shape",
      /abi7_fixture_manifest_generator_shape[\s\S]*?abi7_fixture_manifest_generator_accepts_array[\s\S]*?abi7_fixture_manifest_domains_shape[\s\S]*?abi7_fixture_manifest_domains_accepts_array/u,
      "ABI-7 fixture nested object shape",
    ],
    [
      "--negative-control-abi7-fixture-json-object-shape",
      /abi7_fixture_manifest_not_object[\s\S]*?abi7_fixture_manifest_accepts_array[\s\S]*?abi7_archive_fixture_not_object[\s\S]*?abi7_archive_fixture_accepts_array/u,
      "ABI-7 fixture JSON object shape",
    ],
    [
      "--negative-control-abi7-archive-fixture-entry-shape",
      /abi7_archive_fixture_archives[\s\S]*?abi7_archive_list_accepts_object[\s\S]*?abi7_archive_fixture_archive_shape[\s\S]*?abi7_archive_entry_accepts_string/u,
      "ABI-7 archive fixture entry shape",
    ],
    [
      "--negative-control-abi7-archive-fixture-field-shapes",
      /abi7_archive_fixture_base64[\s\S]*?archive_base64_unchecked[\s\S]*?abi7_archive_fixture_byte_len[\s\S]*?archive_byte_len_unchecked[\s\S]*?abi7_archive_fixture_archive_metadata[\s\S]*?archive_metadata_unchecked[\s\S]*?abi7_archive_fixture_sha256[\s\S]*?archive_sha256_unchecked/u,
      "ABI-7 archive fixture field shapes",
    ],
    [
      "--negative-control-abi7-archive-fixture-canonical-base64",
      /if value != base64\.b64encode\(decoded\)\.decode\("ascii"\):[\s\S]*?if False and value != base64\.b64encode\(decoded\)\.decode\("ascii"\):/u,
      "ABI-7 archive fixture canonical base64",
    ],
    [
      "--negative-control-abi7-fixture-operation-shape",
      /abi7_fixture_manifest_operations_shape[\s\S]*?abi7_manifest_operation_list_accepts_object[\s\S]*?abi7_fixture_manifest_operation_shape[\s\S]*?abi7_manifest_operation_entry_accepts_string/u,
      "ABI-7 fixture operation shape",
    ],
    [
      "--negative-control-abi7-fixture-archive-reference-shape",
      /abi7_fixture_manifest_archive_fixture_shape[\s\S]*?abi7_manifest_archive_reference_accepts_array/u,
      "ABI-7 fixture archive reference shape",
    ],
    [
      "--negative-control-abi7-fixture-strict-json",
      /object_pairs_hook=_reject_duplicate_json_object_pairs[\s\S]*?object_pairs_hook=dict[\s\S]*?parse_constant=_reject_nonfinite_json_constant[\s\S]*?parse_constant=float/u,
      "ABI-7 fixture strict JSON parser",
    ],
    [
      "--negative-control-abi7-fixture-json-size-limit",
      /MAX_ABI7_FIXTURE_JSON_BYTES = 1024 \* 1024[\s\S]*?MAX_ABI7_FIXTURE_JSON_BYTES = 64 \* 1024 \* 1024/u,
      "ABI-7 fixture JSON size limit",
    ],
    [
      "--negative-control-abi7-fixture-file-aliases",
      /abi7_fixture_manifest_file_shape[\s\S]*?abi7_fixture_manifest_file_alias_allowed[\s\S]*?abi7_archive_fixture_file_shape[\s\S]*?abi7_archive_fixture_file_alias_allowed/u,
      "ABI-7 fixture file alias gate",
    ],
    [
      "--negative-control-abi7-fixture-race-and-ancestor-aliases",
      /test_abi7_fixture_manifest_rejects_symlinked_fixture_ancestor_without_path_leak[\s\S]*?test_abi7_archive_fixture_rejects_regular_file_swap_after_preflight_without_path_leak[\s\S]*?override_text_all[\s\S]*?abi7_fixture_race_and_ancestor_missing_negative_control/u,
      "ABI-7 fixture race and ancestor alias regression tests",
    ],
    [
      "--negative-control-abi-fixture-integer-scalars",
      /return isinstance\(value, int\) and not isinstance\(value, bool\) and value == expected[\s\S]*?return value == expected/u,
      "ABI fixture integer scalar exactness",
    ],
    [
      "--negative-control-abi7-fixture-manifest-value-binding",
      /abi7_fixture_manifest_schema[\s\S]*?abi7_fixture_manifest_kind[\s\S]*?abi7_fixture_manifest_bridge_version[\s\S]*?abi7_fixture_manifest_operation_count[\s\S]*?abi7_fixture_manifest_archive_fixture[\s\S]*?abi7_fixture_manifest_generator[\s\S]*?abi7_fixture_manifest_domains[\s\S]*?abi7_fixture_manifest_operations[\s\S]*?f'"\{code\}_disabled",'/u,
      "ABI-7 fixture manifest value binding",
    ],
    [
      "--negative-control-abi7-archive-fixture-value-binding",
      /abi7_archive_fixture_schema[\s\S]*?abi7_archive_fixture_kind[\s\S]*?abi7_archive_fixture_bridge_version[\s\S]*?abi7_archive_fixture_operation_count[\s\S]*?abi7_archive_fixture_operations[\s\S]*?abi7_archive_fixture_missing_archive[\s\S]*?archive_value_binding_disabled_\{index\}/u,
      "ABI-7 archive fixture value binding",
    ],
    [
      "--negative-control-abi7-fixture-unreadable-json",
      /abi7_fixture_manifest_unreadable[\s\S]*?abi7_fixture_manifest_decode_errors_ignored[\s\S]*?abi7_archive_fixture_unreadable[\s\S]*?abi7_archive_fixture_decode_errors_ignored/u,
      "ABI-7 fixture unreadable JSON gate",
    ],
    [
      "--negative-control-abi7-fixture-operation-closed-schema",
      /abi7_fixture_manifest_operation_unexpected_field[\s\S]*?abi7_fixture_manifest_operation_unchecked_field/u,
      "ABI-7 fixture operation closed schema",
    ],
    [
      "--negative-control-abi7-fixture-duplicate-archive",
      /abi7_archive_fixture_duplicate_archive[\s\S]*?abi7_archive_fixture_duplicate_archive_disabled/u,
      "ABI-7 fixture duplicate archive",
    ],
    [
      "--negative-control-lineage-key-release-source-marker-aliases",
      /lineage_key_release_file_shape[\s\S]*?lineage_key_release_file_alias_allowed/u,
      "Reserved-lineage key release source marker alias gate",
    ],
    [
      "--negative-control-lineage-key-release-source-marker-non-utf8-read",
      /except UnicodeDecodeError:[\s\S]*?return None, \[unreadable_error\][\s\S]*?except UnicodeDecodeError:[\s\S]*?return "", \[\]/u,
      "Reserved-lineage key release source marker non-UTF-8 read gate",
    ],
    [
      "--negative-control-release-bundle-compact-generator-log-inventory",
      /"compact_key_generator_log"[\s\S]*?"compactKeyGeneratorLogDisabled"/u,
      "Kagemusha release bundle compact generator log inventory",
    ],
    [
      "--negative-control-kagemusha-readiness-summary-output-private-permissions",
      /kagemusha_production_readiness\.py[\s\S]*?os\.fchmod\(handle\.fileno\(\), 0o600\)[\s\S]*?handle\.fileno\(\)/u,
      "Kagemusha readiness summary output private permissions",
    ],
    [
      "--negative-control-release-bundle-output-private-permissions",
      /kagemusha_release_bundle\.py[\s\S]*?os\.fchmod\(handle\.fileno\(\), 0o600\)[\s\S]*?handle\.fileno\(\)/u,
      "Kagemusha release bundle output private permissions",
    ],
  ];
  for (const [mode, mutationPattern, label] of branchSpecs) {
    const branch = readinessBranch(mode);
    assert.match(branch, /run_negative_control\(/u, `${label} negative control must use the shared runner`);
    assert.match(branch, mutationPattern, `${label} negative control must mutate the guarded source text`);
  }
});

test("Kagemusha staged finalizer negative controls pin execution-report log binding", () => {
  const readiness = source("ci/check_kagemusha_production_readiness.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-lineage-proof-finalizer-execution-log-sha256",
    "--negative-control-compact-key-finalizer-execution-log-sha256",
    "--negative-control-compact-key-finalizer-execution-elapsed-binding",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_production_readiness.sh",
    expectedModes,
    "Kagemusha staged finalizer execution-report guard",
  );
  for (const mode of expectedModes) {
    assert.ok(
      readiness.includes(`ci/check_kagemusha_production_readiness.sh ${mode}`),
      `production readiness workflow requirements must include ${mode}`,
    );
    assert.ok(readiness.includes(`if mode == "${mode}":`), `production readiness guard must implement ${mode}`);
  }

  const readinessBranch = (mode) => {
    const start = readiness.indexOf(`if mode == "${mode}":`);
    assert.notEqual(start, -1, `missing readiness branch ${mode}`);
    const end = readiness.indexOf("\nif mode ==", start + 1);
    return readiness.slice(start, end === -1 ? readiness.length : end);
  };
  const branchSpecs = [
    [
      "--negative-control-lineage-proof-finalizer-execution-log-sha256",
      /log_sha256 must match staged log SHA-256[\s\S]*?log_sha256 may drift from staged log SHA-256/u,
      "Reserved-lineage finalizer execution-report log binding",
    ],
    [
      "--negative-control-compact-key-finalizer-execution-log-sha256",
      /generator_log_sha256 must match staged generator log SHA-256[\s\S]*?generator_log_sha256 may drift from staged generator log SHA-256/u,
      "ABI-7 compact finalizer execution-report log binding",
    ],
    [
      "--negative-control-compact-key-finalizer-execution-elapsed-binding",
      /elapsed_seconds must match staged run report[\s\S]*?elapsed_seconds may drift from staged run report/u,
      "ABI-7 compact finalizer execution-report elapsed binding",
    ],
  ];
  for (const [mode, mutationPattern, label] of branchSpecs) {
    const branch = readinessBranch(mode);
    assert.match(branch, /run_negative_control\(/u, `${label} negative control must use the shared runner`);
    assert.match(branch, mutationPattern, `${label} negative control must mutate the guarded source text`);
  }
});

test("Kagemusha staged runner negative controls pin private output permissions", () => {
  const readiness = source("ci/check_kagemusha_production_readiness.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-lineage-proof-staged-runner-private-permissions",
    "--negative-control-compact-key-staged-runner-private-permissions",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_production_readiness.sh",
    expectedModes,
    "Kagemusha staged runner private-permissions guard",
  );
  for (const mode of expectedModes) {
    assert.ok(
      readiness.includes(`ci/check_kagemusha_production_readiness.sh ${mode}`),
      `production readiness workflow requirements must include ${mode}`,
    );
    assert.ok(readiness.includes(`if mode == "${mode}":`), `production readiness guard must implement ${mode}`);
  }

  const branchSpecs = [
    [
      "--negative-control-lineage-proof-staged-runner-private-permissions",
      /kagemusha_run_lineage_proof_staged\.py[\s\S]*?os\.fchmod\(dir_fd, 0o700\)[\s\S]*?os\.fstat\(dir_fd\)[\s\S]*?os\.fchmod\(file_fd, 0o600\)[\s\S]*?os\.fstat\(file_fd\)[\s\S]*?os\.fchmod\(temp_fd, 0o600\)[\s\S]*?os\.fstat\(temp_fd\)[\s\S]*?os\.fchmod\(log_handle\.fileno\(\), 0o600\)[\s\S]*?log_handle\.fileno\(\)/u,
      "lineage staged runner private-permissions",
    ],
    [
      "--negative-control-compact-key-staged-runner-private-permissions",
      /kagemusha_run_recursive_compact_keygen_staged\.py[\s\S]*?os\.fchmod\(dir_fd, 0o700\)[\s\S]*?os\.fstat\(dir_fd\)[\s\S]*?os\.fchmod\(file_fd, 0o600\)[\s\S]*?os\.fstat\(file_fd\)[\s\S]*?os\.fchmod\(temp_fd, 0o600\)[\s\S]*?os\.fstat\(temp_fd\)[\s\S]*?os\.fchmod\(log_handle\.fileno\(\), 0o600\)[\s\S]*?log_handle\.fileno\(\)/u,
      "compact staged runner private-permissions",
    ],
  ];
  for (const [mode, mutationPattern, label] of branchSpecs) {
    const start = readiness.indexOf(`if mode == "${mode}":`);
    assert.notEqual(start, -1, `missing ${label} branch`);
    const end = readiness.indexOf("\nif mode ==", start + 1);
    const branch = readiness.slice(start, end === -1 ? readiness.length : end);
    assert.match(branch, /run_negative_control\(/u, `${label} negative control must use the shared runner`);
    assert.match(
      branch,
      mutationPattern,
      `${label} negative control must mutate directory, atomic-file, and child-log chmod calls`,
    );
  }
});

test("Kagemusha staged runner negative controls pin heartbeat observability", () => {
  const readiness = source("ci/check_kagemusha_production_readiness.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-lineage-proof-staged-runner-heartbeat",
    "--negative-control-compact-key-staged-runner-heartbeat",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_production_readiness.sh",
    expectedModes,
    "Kagemusha staged runner heartbeat guard",
  );
  for (const mode of expectedModes) {
    assert.ok(
      readiness.includes(`ci/check_kagemusha_production_readiness.sh ${mode}`),
      `production readiness workflow requirements must include ${mode}`,
    );
    assert.ok(readiness.includes(`if mode == "${mode}":`), `production readiness guard must implement ${mode}`);
  }

  const branchSpecs = [
    [
      "--negative-control-lineage-proof-staged-runner-heartbeat",
      /kagemusha_run_lineage_proof_staged\.py[\s\S]*?STAGED_COMMAND_HEARTBEAT_SECONDS = 300\.0[\s\S]*?STAGED_COMMAND_HEARTBEAT_SECONDS = 0\.0[\s\S]*?\[kagemusha-staged-runner\] lineage-proof heartbeat [\s\S]*?\[kagemusha-staged-runner\] lineage-proof quiet /u,
      "lineage staged runner heartbeat",
    ],
    [
      "--negative-control-compact-key-staged-runner-heartbeat",
      /kagemusha_run_recursive_compact_keygen_staged\.py[\s\S]*?STAGED_COMMAND_HEARTBEAT_SECONDS = 300\.0[\s\S]*?STAGED_COMMAND_HEARTBEAT_SECONDS = 0\.0[\s\S]*?\[kagemusha-staged-runner\] compact-keygen heartbeat [\s\S]*?\[kagemusha-staged-runner\] compact-keygen quiet /u,
      "compact staged runner heartbeat",
    ],
  ];
  for (const [mode, mutationPattern, label] of branchSpecs) {
    const start = readiness.indexOf(`if mode == "${mode}":`);
    assert.notEqual(start, -1, `missing ${label} branch`);
    const end = readiness.indexOf("\nif mode ==", start + 1);
    const branch = readiness.slice(start, end === -1 ? readiness.length : end);
    assert.match(branch, /run_negative_control\(/u, `${label} negative control must use the shared runner`);
    assert.match(branch, mutationPattern, `${label} negative control must mutate heartbeat constants and log labels`);
  }
});

test("Kagemusha staged finalizer negative controls pin private output permissions", () => {
  const readiness = source("ci/check_kagemusha_production_readiness.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-lineage-proof-finalizer-private-permissions",
    "--negative-control-compact-key-finalizer-private-permissions",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_production_readiness.sh",
    expectedModes,
    "Kagemusha staged finalizer private-permissions guard",
  );
  for (const mode of expectedModes) {
    assert.ok(
      readiness.includes(`ci/check_kagemusha_production_readiness.sh ${mode}`),
      `production readiness workflow requirements must include ${mode}`,
    );
    assert.ok(readiness.includes(`if mode == "${mode}":`), `production readiness guard must implement ${mode}`);
  }

  const branchSpecs = [
    [
      "--negative-control-lineage-proof-finalizer-private-permissions",
      /kagemusha_finalize_lineage_proof_staged_run\.py[\s\S]*?os\.fchmod\(dir_fd, 0o700\)[\s\S]*?os\.fstat\(dir_fd\)[\s\S]*?os\.fchmod\(dst\.fileno\(\), 0o600\)[\s\S]*?dst\.fileno\(\)/u,
      "lineage staged finalizer private-permissions",
    ],
    [
      "--negative-control-compact-key-finalizer-private-permissions",
      /kagemusha_finalize_recursive_compact_key_staged_run\.py[\s\S]*?os\.fchmod\(dir_fd, 0o700\)[\s\S]*?os\.fstat\(dir_fd\)[\s\S]*?os\.fchmod\(dst\.fileno\(\), 0o600\)[\s\S]*?dst\.fileno\(\)/u,
      "compact staged finalizer private-permissions",
    ],
  ];
  for (const [mode, mutationPattern, label] of branchSpecs) {
    const start = readiness.indexOf(`if mode == "${mode}":`);
    assert.notEqual(start, -1, `missing ${label} branch`);
    const end = readiness.indexOf("\nif mode ==", start + 1);
    const branch = readiness.slice(start, end === -1 ? readiness.length : end);
    assert.match(branch, /run_negative_control\(/u, `${label} negative control must use the shared runner`);
    assert.match(
      branch,
      mutationPattern,
      `${label} negative control must mutate directory and copied-file chmod calls`,
    );
  }
});

test("Kagemusha evidence helper negative controls pin private output permissions", () => {
  const readiness = source("ci/check_kagemusha_production_readiness.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-lineage-proof-helper-output-private-permissions",
    "--negative-control-compact-key-helper-output-private-permissions",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_production_readiness.sh",
    expectedModes,
    "Kagemusha evidence helper private-permissions guard",
  );
  for (const mode of expectedModes) {
    assert.ok(
      readiness.includes(`ci/check_kagemusha_production_readiness.sh ${mode}`),
      `production readiness workflow requirements must include ${mode}`,
    );
    assert.ok(readiness.includes(`if mode == "${mode}":`), `production readiness guard must implement ${mode}`);
  }

  const branchSpecs = [
    [
      "--negative-control-lineage-proof-helper-output-private-permissions",
      /kagemusha_lineage_proof_evidence\.py[\s\S]*?os\.fchmod\(dir_fd, 0o700\)[\s\S]*?os\.fstat\(dir_fd\)[\s\S]*?os\.fchmod\(handle\.fileno\(\), 0o600\)[\s\S]*?handle\.fileno\(\)/u,
      "lineage evidence helper private-permissions",
    ],
    [
      "--negative-control-compact-key-helper-output-private-permissions",
      /kagemusha_recursive_compact_key_evidence\.py[\s\S]*?os\.fchmod\(dir_fd, 0o700\)[\s\S]*?os\.fstat\(dir_fd\)[\s\S]*?os\.fchmod\(handle\.fileno\(\), 0o600\)[\s\S]*?handle\.fileno\(\)/u,
      "compact evidence helper private-permissions",
    ],
  ];
  for (const [mode, mutationPattern, label] of branchSpecs) {
    const start = readiness.indexOf(`if mode == "${mode}":`);
    assert.notEqual(start, -1, `missing ${label} branch`);
    const end = readiness.indexOf("\nif mode ==", start + 1);
    const branch = readiness.slice(start, end === -1 ? readiness.length : end);
    assert.match(branch, /run_negative_control\(/u, `${label} negative control must use the shared runner`);
    assert.match(
      branch,
      mutationPattern,
      `${label} negative control must mutate directory and file chmod calls`,
    );
  }
});

test("recursive Kagemusha payload reducer pins expected-hop and benchmark-name controls", () => {
  const reducer = source("ci/check_kagemusha_recursive_spend_payload_bench.sh");
  const policy = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const benchmarkNameModes = [
    "--negative-control-malformed-payload-benchmark-name",
  ];
  const hopModes = [
    "--negative-control-empty-hop-list",
    "--negative-control-blank-hop-list",
    "--negative-control-non-integer-hop",
    "--negative-control-zero-hop",
    "--negative-control-duplicate-hop",
    "--negative-control-unsorted-hop",
    "--negative-control-leading-zero-hop",
  ];

  assertContainsAll(reducer, benchmarkNameModes, "payload reducer benchmark-name negative controls");
  assertContainsAll(reducer, hopModes, "payload reducer expected-hop negative controls");
  assertContainsAll(
    reducer,
    [
      "def write_named_benchmark(group, benchmark_name):",
      "unexpected recursive Kagemusha {label} benchmark name:",
      'f"{expected_hops[0]}_hop_{payload_baseline}_bytes"',
      "def parse_expected_hops(raw):",
      "expected recursive Kagemusha payload hops must be a non-empty",
      "expected recursive Kagemusha payload hops must be positive integers",
      "expected recursive Kagemusha payload hops must be positive",
      "expected recursive Kagemusha payload hops must be unique",
      "expected recursive Kagemusha payload hops must be sorted in ascending order",
      "expected recursive Kagemusha payload hops must use canonical decimal integers",
      'EXPECTED_HOPS="${KAGEMUSHA_RECURSIVE_SPEND_PAYLOAD_HOPS-1,2,3,5,8,13,21,34,55,64}"',
      'REDUCER_EXPECTED_HOPS=""',
      'REDUCER_EXPECTED_HOPS="1,,2"',
      'REDUCER_EXPECTED_HOPS="1,two,3"',
      'REDUCER_EXPECTED_HOPS="0,1,2"',
      'REDUCER_EXPECTED_HOPS="1,2,2"',
      'REDUCER_EXPECTED_HOPS="1,3,2"',
      'REDUCER_EXPECTED_HOPS="1,02,3"',
    ],
    "payload reducer expected-hop and benchmark-name validation",
  );
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_payload_bench.sh",
    benchmarkNameModes,
    "Kagemusha payload reducer",
  );
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_payload_bench.sh",
    hopModes,
    "Kagemusha payload reducer",
  );
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [
      "--negative-control-payload-benchmark-name-negative-controls-workflow",
      "--negative-control-payload-hop-list-negative-controls-workflow",
    ],
    "Kagemusha policy guard",
  );
  assertContainsAll(
    policy,
    [
      "--negative-control-payload-benchmark-name-negative-controls-workflow",
      "--negative-control-payload-hop-list-negative-controls-workflow",
      "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-malformed-payload-benchmark-name",
      "ci/check_kagemusha_recursive_spend_payload_bench.sh --synthetic-malformed-payload-name-check",
      "ci/check_kagemusha_recursive_spend_payload_bench.sh --negative-control-duplicate-hop",
      "ci/check_kagemusha_recursive_spend_payload_bench.sh --synthetic-duplicate-hop-check",
    ],
    "policy hop-list negative-control meta guard",
  );

  const benchmarkNameBranch = policy.slice(
    policy.indexOf('if mode == "--negative-control-payload-benchmark-name-negative-controls-workflow":'),
    policy.indexOf('if mode == "--negative-control-payload-negative-controls-comment-workflow":'),
  );
  assert.match(
    benchmarkNameBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "payload benchmark-name policy negative control must validate the mutated workflow snapshot",
  );
  assert.match(
    benchmarkNameBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: payload benchmark-name reducer drift was not detected"\)/u,
    "payload benchmark-name policy negative control must only pass after detecting injected drift",
  );

  const branch = policy.slice(
    policy.indexOf('if mode == "--negative-control-payload-hop-list-negative-controls-workflow":'),
    policy.indexOf('if mode == "--negative-control-payload-benchmark-name-negative-controls-workflow":'),
  );
  assert.match(
    branch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "payload hop-list policy negative control must validate the mutated workflow snapshot",
  );
  assert.match(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: payload hop-list reducer drift was not detected"\)/u,
    "payload hop-list policy negative control must only pass after detecting injected drift",
  );
});

test("recursive Kagemusha policy pins payload benchmark append-opening call coverage", () => {
  const policy = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-payload-benchmark-source";
  const callNeedle =
    "kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight(";

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [mode],
    "Kagemusha policy guard",
  );
  assert.match(
    policy,
    /"kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight\("/u,
    "payload benchmark policy coverage must pin the exact append-opening call",
  );
  assertContainsAll(
    policy,
    [
      `'"recursive Kagemusha payload grew at hop {}"'`,
      `'"recursive Kagemusha append transition profile grew at hop {}"'`,
      `'"reserved-lineage recursive Kagemusha payload grew at hop {}"'`,
      `'"reserved-lineage recursive Kagemusha append transition profile grew at hop {}"'`,
    ],
    "payload benchmark policy coverage must pin exact growth assertion messages",
  );
  assert.doesNotMatch(
    policy,
    /"kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight",/u,
    "payload benchmark policy coverage must not accept the shadowable bare append-opening helper name",
  );

  const branch = policy.slice(
    policy.indexOf('if mode == "--negative-control-payload-benchmark-source":'),
    policy.indexOf('if mode == "--negative-control-doc-payload-budget":'),
  );
  assertContainsAll(
    branch,
    [
      "cases = (",
      `"${callNeedle}"`,
      "kagemusha_recursive_spend_transition_profile_append_evidence_without_opening_preflight(",
      `'"recursive Kagemusha payload grew at hop {}"'`,
      `'"recursive Kagemusha append transition profile grew at hop {}"'`,
      `'"reserved-lineage recursive Kagemusha payload grew at hop {}"'`,
      `'"reserved-lineage recursive Kagemusha append transition profile grew at hop {}"'`,
      "for before, after, label in cases:",
      "payload benchmark source drift was not detected for ",
    ],
    "payload benchmark source negative control exact-case guard",
  );
  assert.match(
    branch,
    /source\.replace\(\s*before,\s*after,\s*1\s*\)/u,
    "payload benchmark source negative control must mutate one exact case at a time",
  );
  assert.match(
    branch,
    /if\s+label\s+not\s+in\s+message:[\s\S]*?payload benchmark source drift was not detected for/u,
    "payload benchmark source negative control must require the exact missing-label error",
  );
});

test("recursive Kagemusha policy negative controls pin SDK helper edge exactness", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control";

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [mode],
    "Kagemusha policy guard",
  );
  assert.ok(
    guard.includes('"ci/check_kagemusha_recursive_spend_policy.sh --negative-control"'),
    `policy negative-control inventory must include ${mode}`,
  );

  const branch = guard.slice(
    guard.indexOf('if mode == "--negative-control":'),
    guard.indexOf('if mode == "--negative-control-sdk-selector-edge":'),
  );
  assertContainsAll(
    branch,
    [
      "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
      "[undefined, 1]",
      "javascript/iroha_js/test/package_dist.test.js",
      "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift",
      "KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1, UInt32.max",
      "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
      "KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 to Int.MAX_VALUE",
      "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
      "KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, Integer.MAX_VALUE",
      "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
      "public struct LineageKeyArtifacts: Equatable {",
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
      "class LineageKeyArtifacts internal constructor(",
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
      "public static final class LineageKeyArtifacts {",
      "for target, before, after, label in cases:",
      "SDK helper edge-case drift was not detected for ",
    ],
    "SDK helper edge negative control must cover exact non-C# lineage artifact declarations",
  );
  assert.match(
    branch,
    /for target, before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "SDK helper edge negative control must validate each mutated text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?SDK helper edge-case drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: Reserved-lineage policy drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "SDK helper edge negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK helper edge negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin native host archive caps", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-js-host-kagemusha-archive-cap",
    "--negative-control-python-kagemusha-archive-cap",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    expectedModes,
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  for (const mode of expectedModes) {
    assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
    assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);
  }

  const jsHostArchiveCapBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-host-kagemusha-archive-cap":'),
    guard.indexOf('if mode == "--negative-control-python-recursive-compact-vk-hash":'),
  );
  assert.match(
    jsHostArchiveCapBranch,
    /archive_len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES[\s\S]*?false && archive_len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES[\s\S]*?KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES \+ 1[\s\S]*?KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES/u,
    "JS host archive-cap negative control must weaken both the cap predicate and cap-plus-one fixture",
  );
  assert.match(
    jsHostArchiveCapBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "JS host archive-cap negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsHostArchiveCapBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JS host Kagemusha archive cap drift was not detected"\)/u,
    "JS host archive-cap negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsHostArchiveCapBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS host archive-cap negative control must not unconditionally pass after run_checks",
  );

  const pythonArchiveCapBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-python-kagemusha-archive-cap":'),
    guard.indexOf('if mode == "--negative-control-fixed-window-manifest-digest-splice":'),
  );
  assert.match(
    pythonArchiveCapBranch,
    /archive_len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES[\s\S]*?false && archive_len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES[\s\S]*?KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES \+ 1[\s\S]*?KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES/u,
    "Python host archive-cap negative control must weaken both the cap predicate and cap-plus-one fixture",
  );
  assert.match(
    pythonArchiveCapBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "Python host archive-cap negative control must validate the mutated text snapshot",
  );
  assert.match(
    pythonArchiveCapBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Python Kagemusha archive cap drift was not detected"\)/u,
    "Python host archive-cap negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    pythonArchiveCapBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Python host archive-cap negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin non-C# SDK append cap binding", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-sdk-append-cap-binding";

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [mode],
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
  assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);

  const branch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-append-cap-binding":'),
    guard.indexOf('if mode == "--negative-control-native-output-cap":'),
  );
  assertContainsAll(
    branch,
    [
      "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift",
      "public static let compactTokenMaxHops: UInt32 = 64",
      "return previousHopCount < compactTokenMaxHops",
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
      "const val COMPACT_TOKEN_MAX_HOPS: Int = 64",
      "previousHopCount < COMPACT_TOKEN_MAX_HOPS",
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
      "public static final int COMPACT_TOKEN_MAX_HOPS = 64;",
      "return previousHopCount < COMPACT_TOKEN_MAX_HOPS;",
      "javascript/iroha_js/src/crypto.js",
      "javascript/iroha_js/dist/crypto.js",
      "javascript/iroha_js/src/crypto.browser.js",
      "javascript/iroha_js/dist/crypto.browser.js",
      "export const KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS = 64;",
      "return previousHopCount < KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS;",
      "python/iroha_python/src/iroha_python/kagemusha.py",
      "KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS = 64",
      "previous_hop_count < KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS",
    ],
    "SDK append cap binding negative control must cover every non-C# SDK declaration and comparison",
  );
  assert.doesNotMatch(
    branch,
    /csharp/iu,
    "SDK append cap binding negative control must leave C# mutation work out of scope",
  );
  assert.match(
    branch,
    /for target, before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "SDK append cap binding negative control must validate each mutated SDK text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?SDK append cap binding drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: SDK append cap binding drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "SDK append cap binding negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK append cap binding negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin non-C# native output guard exactness", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-native-output-cap";

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [mode],
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
  assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);

  const branch = guard.slice(
    guard.indexOf('if mode == "--negative-control-native-output-cap":'),
    guard.indexOf('if mode == "--negative-control-shared-fixture-manifest":'),
  );
  assertContainsAll(
    branch,
    [
      "javascript/iroha_js/src/crypto.js",
      "export const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;",
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaCompactPaymentTokenProver.java",
      "static void requireNativeInput(final byte[] archive, final String archiveName)",
      "static boolean isValidNoritoArchive(final byte[] output)",
      "static boolean hasNonEmptyNoritoPayload(final byte[] output)",
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveCompactPaymentTokenProver.java",
      "private static void requireNativeInput(final byte[] archive, final String archiveName)",
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCompactPaymentTokenProver.kt",
      "internal fun requireNativeInput(archive: ByteArray?, archiveName: String): ByteArray",
      "internal fun isValidNoritoArchive(output: ByteArray?): Boolean",
      "internal fun hasNonEmptyNoritoPayload(output: ByteArray?): Boolean =",
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveCompactPaymentTokenProver.kt",
      "private fun requireNativeInput(archive: ByteArray?, archiveName: String): ByteArray",
      "python/iroha_python/src/iroha_python/kagemusha.py",
      "def _archive_bytes_named(archive: BytesLike, name: str) -> bytes:",
      "def _norito_archive_bytes_named(archive: BytesLike, name: str) -> bytes:",
    ],
    "native output cap negative control must cover exact non-C# helper declarations",
  );
  assert.doesNotMatch(
    branch,
    /csharp/iu,
    "native output cap negative control must leave C# mutation work out of scope",
  );
  assert.match(
    branch,
    /for target, before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "native output cap negative control must validate each mutated non-C# text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?native output cap drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: native output cap drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "native output cap negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "native output cap negative control must not unconditionally pass after run_checks",
  );

  const abi7Modes = [
    "--negative-control-shared-abi7-fixture-manifest",
    "--negative-control-shared-abi7-archive-fixture",
    "--negative-control-shared-abi7-sdk-manifest-coverage",
  ];
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    abi7Modes,
    "Kagemusha policy guard",
  );
  for (const abi7Mode of abi7Modes) {
    assert.ok(
      inventoryModes.includes(abi7Mode),
      `policy negative-control inventory must include ${abi7Mode}`,
    );
    assert.ok(guard.includes(`if mode == "${abi7Mode}":`), `policy guard must implement ${abi7Mode}`);
  }
  assertContainsAll(
    guard,
    [
      "SHARED_ABI7_FIXTURE_PATH",
      "SHARED_ABI7_ARCHIVE_FIXTURE_PATH",
      "SHARED_ABI7_FIXTURE_COVERAGE",
      "check_shared_abi7_fixture_manifest()",
      "check_shared_abi7_archive_fixture_manifest()",
      "kagemusha_recursive_spend_abi7_archive_fixture_matches_python_native_bridge",
      "_shared_recursive_spend_abi7_manifest",
      "test_recursive_kagemusha_shared_abi7_fixture_manifest_matches_archives_and_generator",
      "sharedRecursiveSpendAbi7Manifest",
      "Kagemusha recursive spend shared ABI-7 fixture manifest matches archive fixture",
      "testSharedRecursiveSpendAbi7ManifestMatchesArchiveFixture",
      "ABI 7 fixture manifest matches archive fixture",
      "sharedRecursiveSpendAbi7FixtureManifestMatchesArchiveFixture",
      "assert set(manifest) ==",
      "len(archive_entries) == len(expected_operations)",
      "hashlib.sha256(archive_bytes).hexdigest()",
      "Object.keys(manifest).sort()",
      "archiveFixture.archives.length, expectedOperations.size",
      "createHash(\"sha256\").update(archiveBytes).digest(\"hex\")",
      "Set(manifest.keys)",
      "archives.count, expectedOperations.count",
      "SHA256.hash(data: archiveBytes)",
      "manifest.keys",
      "expectedOperations.size, archives.size",
      "sha256Hex(archiveBytes)",
      "assertKeySet(",
      "archives.size() == expectedNames.size()",
    ],
    "ABI-7 shared fixture policy coverage",
  );
  const abi7ManifestBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-shared-abi7-fixture-manifest":'),
    guard.indexOf('if mode == "--negative-control-shared-abi7-archive-fixture":'),
  );
  assert.match(
    abi7ManifestBranch,
    /"operation_count": 5[\s\S]*?"operation_count": 4[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "ABI-7 fixture manifest negative control must mutate and validate the manifest snapshot",
  );
  const abi7ArchiveBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-shared-abi7-archive-fixture":'),
    guard.indexOf('if mode == "--negative-control-shared-abi7-sdk-manifest-coverage":'),
  );
  assert.match(
    abi7ArchiveBranch,
    /d493b27708e00c23ed9be2a040695a49e368a4b664c6330012f15162d7f5c01e[\s\S]*?0093b27708e00c23ed9be2a040695a49e368a4b664c6330012f15162d7f5c01e[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "ABI-7 archive fixture negative control must mutate and validate the archive snapshot",
  );
  const abi7SdkCoverageBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-shared-abi7-sdk-manifest-coverage":'),
    guard.indexOf('if mode == "--negative-control-data-model-append-cap-boundary":'),
  );
  assertContainsAll(
    abi7SdkCoverageBranch,
    [
      "python/iroha_python/tests/kagemusha_test.py",
      "_shared_recursive_spend_abi7_manifest",
      "test_recursive_kagemusha_shared_abi7_fixture_manifest_matches_archives_and_generator",
      "assert set(manifest) ==",
      "assert set(archive) ==",
      "len(archive_entries) == len(expected_operations)",
      "hashlib.sha256(archive_bytes).hexdigest()",
      "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
      "sharedRecursiveSpendAbi7Manifest",
      "Kagemusha recursive spend shared ABI-7 fixture manifest matches archive fixture",
      "Object.keys(manifest).sort()",
      "Object.keys(archive).sort()",
      "archiveFixture.archives.length, expectedOperations.size",
      "createHash(\"sha256\").update(archiveBytes).digest(\"hex\")",
      "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift",
      "sharedRecursiveSpendAbi7Manifest",
      "testSharedRecursiveSpendAbi7ManifestMatchesArchiveFixture",
      "Set(manifest.keys)",
      "Set(archive.keys)",
      "archives.count, expectedOperations.count",
      "SHA256.hash(data: archiveBytes)",
      "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendRequestCodecsTest.kt",
      "ABI 7 fixture manifest matches archive fixture",
      "manifest.keys",
      "archive.keys",
      "expectedOperations.size, archives.size",
      "sha256Hex(archiveBytes)",
      "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
      "sharedRecursiveSpendAbi7FixtureManifestMatchesArchiveFixture",
      "sharedRecursiveSpendAbi7Manifest",
      "assertKeySet(",
      "byte_len\\\", \\\"sha256_hex\\\", \\\"bytes_base64",
      "archives.size() == expectedNames.size()",
      "archiveBytes.length",
    ],
    "ABI-7 SDK manifest coverage negative control must cover every non-C# SDK test surface",
  );
  assert.match(
    abi7SdkCoverageBranch,
    /for target, needle in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "ABI-7 SDK manifest coverage negative control must validate each mutated SDK snapshot",
  );
  assert.match(
    abi7SdkCoverageBranch,
    /is missing shared recursive spend ABI-7 fixture coverage[\s\S]*?shared ABI-7 SDK manifest coverage drift was not detected for[\s\S]*?raise\s+SystemExit\(0\)/u,
    "ABI-7 SDK manifest coverage negative control must only pass after detecting all injected drifts",
  );
});

test("recursive Kagemusha policy negative controls pin lineage accumulator coverage", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-core-lineage-profile-split",
    "--negative-control-core-proof-chain-accumulator",
    "--negative-control-core-fixed-window-table-base-accumulator",
    "--negative-control-core-append-boundary-accumulator",
    "--negative-control-core-previous-accumulator-boundary",
    "--negative-control-core-append-boundary-opening-preflight-refresh",
    "--negative-control-core-append-boundary-current-opening-refresh",
    "--negative-control-core-append-boundary-public-inputs-refresh",
    "--negative-control-core-append-boundary-verifier-context-refresh",
    "--negative-control-core-append-boundary-hop-count-refresh",
    "--negative-control-core-resulting-accumulator-boundary",
    "--negative-control-core-append-boundary-digest-match",
    "--negative-control-core-append-boundary-context-matches",
    "--negative-control-core-append-digest-unchecked-surface",
    "--negative-control-core-append-digest-wrapper-bypass",
    "--negative-control-core-append-boundary-profile-comparison",
    "--negative-control-data-model-self-consistent-boundary",
    "--negative-control-data-model-proof-public-input-circuit-binding",
    "--negative-control-data-model-semantic-proof-append-opening",
    "--negative-control-data-model-public-input-one-hop-append-opening",
    "--negative-control-data-model-generic-proof-scalar-projection",
    "--negative-control-data-model-spend-proof-artifact-circuit-gates",
    "--negative-control-data-model-previous-proof-opening-bundle-binding",
    "--negative-control-data-model-previous-proof-field-binding",
    "--negative-control-data-model-previous-proof-stale-hash-fixture",
    "--negative-control-core-recursive-public-input-schema-order",
    "--negative-control-core-recursive-public-input-index-map",
    "--negative-control-core-recursive-public-input-value-order",
    "--negative-control-core-recursive-public-input-nonzero-groups",
    "--negative-control-core-recursive-append-semantic-nonzero-groups",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    expectedModes,
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  for (const mode of expectedModes) {
    assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
    assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);
  }
  assertContainsAll(
    guard,
    [
      "KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1",
      "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-64x4",
      "pub const KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOWS: usize = 64;",
      "pub const KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOW_BITS: usize = 4;",
    ],
    "Kagemusha policy verifier witness profile source pins",
  );

  const selfConsistentBoundaryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-self-consistent-boundary":'),
    guard.indexOf('if mode == "--negative-control-data-model-transition-profile-current-hop-sets":'),
  );
  assertContainsAll(
    selfConsistentBoundaryBranch,
    [
      "cases = (",
      "fn assert_self_consistent_forged_boundary_rejected(",
      "fn assert_profile_bound_forged_boundary_rejected(",
      "zero_chain_asset_boundary.chain_asset_binding_digest = [0u8; Hash::LENGTH];",
      "unchecked-chain-asset",
      "zero_final_note_boundary.final_note_binding_digest = [0u8; Hash::LENGTH];",
      "unchecked-final-note",
      "for before, after, label in cases:",
      "self-consistent append-boundary drift was not detected for ",
    ],
    "data-model self-consistent boundary negative control must pin exact forged-boundary labels",
  );
  assert.match(
    selfConsistentBoundaryBranch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "data-model self-consistent boundary negative control must validate each mutated text snapshot",
  );
  assert.match(
    selfConsistentBoundaryBranch,
    /if label not in message:[\s\S]*?self-consistent append-boundary drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: self-consistent append-boundary drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "data-model self-consistent boundary negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    selfConsistentBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "data-model self-consistent boundary negative control must not unconditionally pass after run_checks",
  );

  const proofPublicInputCircuitBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-proof-public-input-circuit-binding":'),
    guard.indexOf('if mode == "--negative-control-data-model-previous-proof-field-binding":'),
  );
  assert.match(
    proofPublicInputCircuitBranch,
    /expected\.append_opening_preflight_digest == \[0u8; Hash::LENGTH\][\s\S]*?expected\.append_opening_preflight_digest != \[0u8; Hash::LENGTH\]/u,
    "proof public-input circuit binding negative control must flip append preflight boundary routing",
  );
  assert.match(
    proofPublicInputCircuitBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "proof public-input circuit binding negative control must validate the mutated text snapshot",
  );
  assert.match(
    proofPublicInputCircuitBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: proof public-input circuit binding drift was not detected"\)/u,
    "proof public-input circuit binding negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    proofPublicInputCircuitBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "proof public-input circuit binding negative control must not unconditionally pass after run_checks",
  );

  const semanticProofAppendOpeningBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-semantic-proof-append-opening":'),
    guard.indexOf('if mode == "--negative-control-data-model-public-input-one-hop-append-opening":'),
  );
  assert.match(
    semanticProofAppendOpeningBranch,
    /accumulator\.append_opening_preflight_digest != \[0u8; Hash::LENGTH\][\s\S]*?false && accumulator\.append_opening_preflight_digest/u,
    "semantic proof append-opening negative control must bypass the semantic-circuit append-opening guard",
  );
  assert.match(
    semanticProofAppendOpeningBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "semantic proof append-opening negative control must validate the mutated text snapshot",
  );
  assert.match(
    semanticProofAppendOpeningBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: semantic proof append-opening drift was not detected"\)/u,
    "semantic proof append-opening negative control must only pass after detecting injected drift",
  );

  const oneHopAppendOpeningBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-public-input-one-hop-append-opening":'),
    guard.indexOf('if mode == "--negative-control-data-model-generic-proof-scalar-projection":'),
  );
  assert.match(
    oneHopAppendOpeningBranch,
    /append_opening_preflight_digest != \[0u8; Hash::LENGTH\] && self\.hop_count <= 1[\s\S]*?append_opening_preflight_digest != \[0u8; Hash::LENGTH\] && self\.hop_count == 0/u,
    "one-hop append-opening negative control must relax the impossible one-hop public-input guard",
  );
  assert.match(
    oneHopAppendOpeningBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "one-hop append-opening negative control must validate the mutated text snapshot",
  );
  assert.match(
    oneHopAppendOpeningBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: one-hop append-opening public-input drift was not detected"\)/u,
    "one-hop append-opening negative control must only pass after detecting injected drift",
  );

  const genericProofScalarBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-generic-proof-scalar-projection":'),
    guard.indexOf('if mode == "--negative-control-data-model-spend-proof-artifact-circuit-gates":'),
  );
  assert.match(
    genericProofScalarBranch,
    /self\.public_inputs\\n[\s\S]*?recursive_verifier_scalar_projection_digest[\s\S]*?\[0u8; Hash::LENGTH\]/u,
    "generic proof scalar-projection negative control must replace the generic-circuit scalar binding with zero",
  );
  assert.match(
    genericProofScalarBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "generic proof scalar-projection negative control must validate the mutated text snapshot",
  );
  assert.match(
    genericProofScalarBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: generic proof scalar-projection drift was not detected"\)/u,
    "generic proof scalar-projection negative control must only pass after detecting injected drift",
  );

  const spendProofArtifactCircuitBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-spend-proof-artifact-circuit-gates":'),
    guard.indexOf('if mode == "--negative-control-data-model-previous-proof-opening-bundle-binding":'),
  );
  assert.match(
    spendProofArtifactCircuitBranch,
    /&& public_inputs\.append_boundary_digest == \[0u8; Hash::LENGTH\][\s\S]*?&& false/u,
    "spend proof artifact circuit-gate negative control must remove the lineage append-boundary guard",
  );
  assert.match(
    spendProofArtifactCircuitBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "spend proof artifact circuit-gate negative control must validate the mutated text snapshot",
  );
  assert.match(
    spendProofArtifactCircuitBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: spend proof artifact circuit gate drift was not detected"\)/u,
    "spend proof artifact circuit-gate negative control must only pass after detecting injected drift",
  );

  const previousProofOpeningBundleBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-previous-proof-opening-bundle-binding":'),
    guard.indexOf('if mode == "--negative-control-data-model-previous-proof-field-binding":'),
  );
  assert.match(
    previousProofOpeningBundleBranch,
    /previous_bundle\.validate_public_input_binding\(\)\?;[\s\S]*?""/u,
    "previous-proof opening bundle-binding negative control must remove the bundle binding call",
  );
  assert.match(
    previousProofOpeningBundleBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "previous-proof opening bundle-binding negative control must validate the mutated text snapshot",
  );
  assert.match(
    previousProofOpeningBundleBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: previous-proof opening bundle binding drift was not detected"\)/u,
    "previous-proof opening bundle-binding negative control must only pass after detecting injected drift",
  );

  const previousProofFieldBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-previous-proof-field-binding":'),
    guard.indexOf('if mode == "--negative-control-data-model-previous-proof-stale-hash-fixture":'),
  );
  assert.match(
    previousProofFieldBranch,
    /ensure_recursive_spend_previous_proof_matches[\s\S]*?ensure_field!\(folded_public_inputs_hash\)/u,
    "previous-proof field binding negative control must target the previous-proof folded hash comparison",
  );
  assert.match(
    previousProofFieldBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "previous-proof field binding negative control must validate the mutated text snapshot",
  );
  assert.match(
    previousProofFieldBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: previous-proof field binding drift was not detected"\)/u,
    "previous-proof field binding negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    previousProofFieldBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "previous-proof field binding negative control must not unconditionally pass after run_checks",
  );
  assertContainsAll(
    guard,
    [
      "spliced previous proof folded public-input hash",
      'field: "previous_recursive_proof.folded_public_inputs_hash"',
    ],
    "previous-proof folded hash adversarial fixture policy coverage",
  );
  const previousProofStaleHashBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-previous-proof-stale-hash-fixture":'),
    guard.indexOf('if mode == "--negative-control-core-append-cap-boundary":'),
  );
  assert.match(
    previousProofStaleHashBranch,
    /recursive-spend-stale-previous-proof-public-input-hash[\s\S]*?recursive-spend-previous-proof-public-input-hash/u,
    "previous-proof stale hash negative control must mutate the stale cached hash fixture",
  );
  assert.match(
    previousProofStaleHashBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "previous-proof stale hash negative control must validate the mutated text snapshot",
  );
  assert.match(
    previousProofStaleHashBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: stale previous-proof hash fixture drift was not detected"\)/u,
    "previous-proof stale hash negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    previousProofStaleHashBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "previous-proof stale hash negative control must not unconditionally pass after run_checks",
  );

  const profileSplitBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-lineage-profile-split":'),
    guard.indexOf('if mode == "--negative-control-core-proof-chain-accumulator":'),
  );
  assertContainsAll(
    profileSplitBranch,
    [
      "pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_CIRCUIT_ID: &str =",
      "pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_CIRCUIT_ID: &str =",
      "pub fn kagemusha_recursive_spend_lineage_append_vk_record(",
      'err.contains("is not `")\\n                && err.contains(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_CIRCUIT_ID)',
      'err.contains("is not `")\\n                && err.contains(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_CIRCUIT_ID)',
      "Reserved-lineage one-hop and append verifier records must coexist under distinct circuit ids",
      "RecursiveCompactKeyArtifacts(KagemushaRecursiveCompactKeyArtifactsArgs),",
      "LineageRecord(KagemushaLineageRecordArgs),",
      "pub struct KagemushaRecursiveCompactKeyArtifactsArgs {",
      "pub struct KagemushaLineageRecordArgs {",
      "for target, before, after, label in cases:",
      "Reserved-lineage profile split drift was not detected for ",
    ],
    "Reserved-lineage profile split negative control must mutate exact core and CLI coverage labels",
  );
  assert.match(
    profileSplitBranch,
    /for target, before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "Reserved-lineage profile split negative control must validate each mutated text snapshot",
  );
  assert.match(
    profileSplitBranch,
    /if label not in message:[\s\S]*?Reserved-lineage profile split drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: Reserved-lineage profile split drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "Reserved-lineage profile split negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    profileSplitBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Reserved-lineage profile split negative control must not pass after an undetected run_checks result",
  );

  const proofChainBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-proof-chain-accumulator":'),
    guard.indexOf('if mode == "--negative-control-core-fixed-window-table-base-accumulator":'),
  );
  assert.match(
    proofChainBranch,
    /proof-byte splice is bound into accumulator state[\s\S]*?proof-byte splice may be detached from accumulator state/u,
    "proof-chain accumulator negative control must mutate the guarded coverage",
  );
  assert.match(
    proofChainBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "proof-chain accumulator negative control must validate the mutated text snapshot",
  );
  assert.match(
    proofChainBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: proof-chain accumulator drift was not detected"\)/u,
    "proof-chain accumulator negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    proofChainBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "proof-chain accumulator negative control must not unconditionally pass after run_checks",
  );

  const tableBaseBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-fixed-window-table-base-accumulator":'),
    guard.indexOf('if mode == "--negative-control-core-append-boundary-accumulator":'),
  );
  assert.match(
    tableBaseBranch,
    /per-hop fixed-window table-base digest must stream across append[\s\S]*?per-hop fixed-window table-base digest may be detached from append/u,
    "fixed-window table-base accumulator negative control must mutate the guarded coverage",
  );
  assert.match(
    tableBaseBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "fixed-window table-base accumulator negative control must validate the mutated text snapshot",
  );
  assert.match(
    tableBaseBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: fixed-window table-base accumulator drift was not detected"\)/u,
    "fixed-window table-base accumulator negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    tableBaseBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "fixed-window table-base accumulator negative control must not unconditionally pass after run_checks",
  );

  const appendBoundaryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-boundary-accumulator":'),
    guard.indexOf('if mode == "--negative-control-core-previous-accumulator-boundary":'),
  );
  assert.match(
    appendBoundaryBranch,
    /append-boundary digest must not feed back into the accumulator digest[\s\S]*?append-boundary digest may feed back into the accumulator digest/u,
    "append-boundary accumulator negative control must mutate the guarded coverage",
  );
  assert.match(
    appendBoundaryBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append-boundary accumulator negative control must validate the mutated text snapshot",
  );
  assert.match(
    appendBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append-boundary accumulator drift was not detected"\)/u,
    "append-boundary accumulator negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    appendBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append-boundary accumulator negative control must not unconditionally pass after run_checks",
  );

  const previousAccumulatorBoundaryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-previous-accumulator-boundary":'),
    guard.indexOf('if mode == "--negative-control-core-append-boundary-opening-preflight-refresh":'),
  );
  assert.match(
    previousAccumulatorBoundaryBranch,
    /field: "append_boundary\.previous_accumulator_digest"[\s\S]*?field: "append_boundary\.previous_accumulator_digest_unchecked"/u,
    "previous accumulator boundary negative control must mutate the guarded coverage",
  );
  assert.match(
    previousAccumulatorBoundaryBranch,
    /refresh_append_boundary_digest\(&mut self_consistent_forged_previous\);[\s\S]*?let _unchecked_previous_boundary = &self_consistent_forged_previous;/u,
    "previous accumulator boundary negative control must remove the self-consistent forged-boundary digest refresh",
  );
  assert.match(
    previousAccumulatorBoundaryBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "previous accumulator boundary negative control must validate the mutated text snapshot",
  );
  assert.match(
    previousAccumulatorBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: previous accumulator boundary drift was not detected"\)/u,
    "previous accumulator boundary negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    previousAccumulatorBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "previous accumulator boundary negative control must not unconditionally pass after run_checks",
  );

  const openingPreflightBoundaryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-boundary-opening-preflight-refresh":'),
    guard.indexOf('if mode == "--negative-control-core-append-boundary-current-opening-refresh":'),
  );
  assertContainsAll(
    openingPreflightBoundaryBranch,
    [
      "cases = (",
      'field: "append_boundary.append_opening_preflight_digest"',
      'field: "append_boundary.append_opening_preflight_digest_unchecked"',
      "refresh_append_boundary_digest(&mut self_consistent_forged_opening);",
      "let _unchecked_opening_preflight = &self_consistent_forged_opening;",
      "self_consistent_forged_opening\\n                .validate_against_transition_profile",
      "self_consistent_forged_opening\\n                .validate_context",
      "for before, after, label in cases:",
      "append-boundary opening preflight refresh drift was not detected for ",
    ],
    "append-opening preflight boundary negative control must pin exact field, refresh, and profile-match labels",
  );
  assert.match(
    openingPreflightBoundaryBranch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append-opening preflight boundary negative control must validate each mutated text snapshot",
  );
  assert.match(
    openingPreflightBoundaryBranch,
    /if label not in message:[\s\S]*?append-boundary opening preflight refresh drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: append-boundary opening preflight refresh drift was not detected"\s*\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "append-opening preflight boundary negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    openingPreflightBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append-opening preflight boundary negative control must not unconditionally pass after run_checks",
  );

  const currentOpeningBoundaryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-boundary-current-opening-refresh":'),
    guard.indexOf('if mode == "--negative-control-core-append-boundary-public-inputs-refresh":'),
  );
  assertContainsAll(
    currentOpeningBoundaryBranch,
    [
      'field: "append_boundary.current_hop_opening_aggregate_digest"',
      'field: "append_boundary.current_hop_opening_aggregate_digest_unchecked"',
      "refresh_append_boundary_digest(&mut self_consistent_forged_current_opening);",
      "let _unchecked_current_opening = &self_consistent_forged_current_opening;",
      "self_consistent_forged_current_opening\\n                .validate_against_transition_profile",
      "self_consistent_forged_current_opening\\n                .validate_context",
      "pub fn validate_against_transition_profile(",
      "pub fn validate_against_transition_profile_unchecked(",
      "for before, after, label in cases:",
      "append-boundary current opening refresh drift was not detected for ",
    ],
    "current-hop opening boundary negative control must mutate exact guarded coverage labels",
  );
  assert.match(
    currentOpeningBoundaryBranch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "current-hop opening boundary negative control must validate each mutated text snapshot",
  );
  assert.match(
    currentOpeningBoundaryBranch,
    /if label not in message:[\s\S]*?append-boundary current opening refresh drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: append-boundary current opening refresh drift was not detected"\s*\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "current-hop opening boundary negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    currentOpeningBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "current-hop opening boundary negative control must not unconditionally pass after run_checks",
  );

  const publicInputsBoundaryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-boundary-public-inputs-refresh":'),
    guard.indexOf('if mode == "--negative-control-core-append-boundary-verifier-context-refresh":'),
  );
  assert.match(
    publicInputsBoundaryBranch,
    /field: "append_boundary\.resulting_public_inputs_hash"[\s\S]*?field: "append_boundary\.resulting_public_inputs_hash_unchecked"/u,
    "append-boundary public-inputs negative control must mutate the guarded coverage",
  );
  assert.match(
    publicInputsBoundaryBranch,
    /refresh_append_boundary_digest\(&mut self_consistent_forged_public_inputs\);[\s\S]*?let _unchecked_public_inputs = &self_consistent_forged_public_inputs;/u,
    "append-boundary public-inputs negative control must remove the self-consistent forged-boundary digest refresh",
  );
  assert.match(
    publicInputsBoundaryBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append-boundary public-inputs negative control must validate the mutated text snapshot",
  );
  assert.match(
    publicInputsBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: append-boundary public inputs refresh drift was not detected"\s*\)/u,
    "append-boundary public-inputs negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    publicInputsBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append-boundary public-inputs negative control must not unconditionally pass after run_checks",
  );

  const verifierContextBoundaryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-boundary-verifier-context-refresh":'),
    guard.indexOf('if mode == "--negative-control-core-append-boundary-hop-count-refresh":'),
  );
  assert.match(
    verifierContextBoundaryBranch,
    /field: "append_boundary\.verifier_params_fingerprint"[\s\S]*?field: "append_boundary\.verifier_params_fingerprint_unchecked"/u,
    "append-boundary verifier-context negative control must mutate the guarded coverage",
  );
  assert.match(
    verifierContextBoundaryBranch,
    /refresh_append_boundary_digest\(&mut self_consistent_forged_verifier_context\);[\s\S]*?let _unchecked_verifier_context = &self_consistent_forged_verifier_context;/u,
    "append-boundary verifier-context negative control must remove the self-consistent forged-boundary digest refresh",
  );
  assert.match(
    verifierContextBoundaryBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append-boundary verifier-context negative control must validate the mutated text snapshot",
  );
  assert.match(
    verifierContextBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: append-boundary verifier context refresh drift was not detected"\s*\)/u,
    "append-boundary verifier-context negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    verifierContextBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append-boundary verifier-context negative control must not unconditionally pass after run_checks",
  );

  const hopCountBoundaryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-boundary-hop-count-refresh":'),
    guard.indexOf('if mode == "--negative-control-core-resulting-accumulator-boundary":'),
  );
  assert.match(
    hopCountBoundaryBranch,
    /field: "append_boundary\.hop_count"[\s\S]*?field: "append_boundary\.hop_count_unchecked"/u,
    "append-boundary hop-count negative control must mutate the guarded coverage",
  );
  assert.match(
    hopCountBoundaryBranch,
    /refresh_append_boundary_digest\(&mut self_consistent_forged_hop_count\);[\s\S]*?let _unchecked_hop_count = &self_consistent_forged_hop_count;/u,
    "append-boundary hop-count negative control must remove the self-consistent forged-boundary digest refresh",
  );
  assert.match(
    hopCountBoundaryBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append-boundary hop-count negative control must validate the mutated text snapshot",
  );
  assert.match(
    hopCountBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: append-boundary hop-count refresh drift was not detected"\s*\)/u,
    "append-boundary hop-count negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    hopCountBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append-boundary hop-count negative control must not unconditionally pass after run_checks",
  );

  const resultingAccumulatorBoundaryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-resulting-accumulator-boundary":'),
    guard.indexOf('if mode == "--negative-control-core-append-boundary-digest-match":'),
  );
  assert.match(
    resultingAccumulatorBoundaryBranch,
    /append_boundary\.resulting_accumulator_digest != expected_accumulator_digest[\s\S]*?append_boundary\.resulting_accumulator_digest == expected_accumulator_digest/u,
    "resulting accumulator boundary negative control must mutate the guarded coverage",
  );
  assert.match(
    resultingAccumulatorBoundaryBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "resulting accumulator boundary negative control must validate the mutated text snapshot",
  );
  assert.match(
    resultingAccumulatorBoundaryBranch,
    /label = "append_boundary\.resulting_accumulator_digest != expected_accumulator_digest"[\s\S]*?if label not in message:/u,
    "resulting accumulator boundary negative control must require the exact missing comparator label",
  );
  assert.match(
    resultingAccumulatorBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: resulting accumulator boundary drift was not detected"\)/u,
    "resulting accumulator boundary negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    resultingAccumulatorBoundaryBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "resulting accumulator boundary negative control must not unconditionally pass after run_checks",
  );

  const appendBoundaryDigestMatchBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-boundary-digest-match":'),
    guard.indexOf('if mode == "--negative-control-core-append-boundary-context-matches":'),
  );
  assert.match(
    appendBoundaryDigestMatchBranch,
    /append_boundary\.append_boundary_digest != accumulator\.append_boundary_digest[\s\S]*?append_boundary\.append_boundary_digest == accumulator\.append_boundary_digest/u,
    "append-boundary digest match negative control must mutate the guarded coverage",
  );
  assert.match(
    appendBoundaryDigestMatchBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append-boundary digest match negative control must validate the mutated text snapshot",
  );
  assert.match(
    appendBoundaryDigestMatchBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append-boundary digest match drift was not detected"\)/u,
    "append-boundary digest match negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    appendBoundaryDigestMatchBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append-boundary digest match negative control must not unconditionally pass after run_checks",
  );

  const appendBoundaryContextMatchesBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-boundary-context-matches":'),
    guard.indexOf('if mode == "--negative-control-core-vesta-ipa-h-fold":'),
  );
  for (const [before, after] of [
    [
      /append_boundary\.transition_profile_binding_digest\\n            != accumulator\.transition_profile_binding_digest/u,
      /append_boundary\.transition_profile_binding_digest\\n            == accumulator\.transition_profile_binding_digest/u,
    ],
    [
      /append_boundary\.chain_asset_binding_digest != expected_chain_asset_binding_digest/u,
      /append_boundary\.chain_asset_binding_digest == expected_chain_asset_binding_digest/u,
    ],
    [
      /append_boundary\.final_note_binding_digest != expected_final_note_binding_digest/u,
      /append_boundary\.final_note_binding_digest == expected_final_note_binding_digest/u,
    ],
    [
      /append_boundary\.resulting_public_inputs_hash != expected_public_inputs_hash/u,
      /append_boundary\.resulting_public_inputs_hash == expected_public_inputs_hash/u,
    ],
  ]) {
    assert.match(
      appendBoundaryContextMatchesBranch,
      before,
      "append-boundary context match negative control must name the guarded comparator",
    );
    assert.match(
      appendBoundaryContextMatchesBranch,
      after,
      "append-boundary context match negative control must flip the guarded comparator",
    );
  }
  assert.match(
    appendBoundaryContextMatchesBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append-boundary context match negative control must validate the mutated text snapshot",
  );
  assert.match(
    appendBoundaryContextMatchesBranch,
    /cases\s*=\s*\([\s\S]*?append_boundary\.transition_profile_binding_digest[\s\S]*?append_boundary\.chain_asset_binding_digest[\s\S]*?append_boundary\.final_note_binding_digest[\s\S]*?append_boundary\.resulting_public_inputs_hash[\s\S]*?for before, after, label in cases:[\s\S]*?if label not in message:[\s\S]*?append-boundary context match drift was not detected for/u,
    "append-boundary context match negative control must check each context comparator independently",
  );
  assert.match(
    appendBoundaryContextMatchesBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?if label not in message:[\s\S]*?first_message = message[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: append-boundary context match drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "append-boundary context match negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    appendBoundaryContextMatchesBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append-boundary context match negative control must not unconditionally pass after run_checks",
  );

  assert.match(
    guard,
    /def check_append_digest_helpers_are_checked\(\):[\s\S]*?unchecked append digest helper must remain private[\s\S]*?append opening preflight public digest wrapper[\s\S]*?preflight\.validate_context\(\)\?;[\s\S]*?Ok\(preflight\.append_opening_preflight_digest\)[\s\S]*?append boundary public digest wrapper[\s\S]*?boundary\.validate_context\(\)\?;[\s\S]*?Ok\(boundary\.append_boundary_digest\)/u,
    "policy guard must pin private unchecked append digest helpers and checked public wrappers",
  );

  const uncheckedSurfaceBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-digest-unchecked-surface":'),
    guard.indexOf('if mode == "--negative-control-core-append-digest-wrapper-bypass":'),
  );
  assert.match(
    uncheckedSurfaceBranch,
    /fn \{helper\}\([\s\S]*?pub fn \{helper\}\(/u,
    "append digest unchecked surface negative control must expose private helpers",
  );
  assert.match(
    uncheckedSurfaceBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append digest unchecked surface negative control must validate the mutated text snapshot",
  );
  assert.match(
    uncheckedSurfaceBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append digest unchecked surface drift was not detected"\)/u,
    "append digest unchecked surface negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    uncheckedSurfaceBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append digest unchecked surface negative control must not unconditionally pass after run_checks",
  );

  const wrapperBypassBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-digest-wrapper-bypass":'),
    guard.indexOf('if mode == "--negative-control-core-vesta-ipa-h-fold":'),
  );
  assert.match(
    wrapperBypassBranch,
    /preflight\.validate_context\(\)\?;\\n"\s*"    Ok\(preflight\.append_opening_preflight_digest\)[\s\S]*?kagemusha_recursive_spend_lineage_append_opening_preflight_digest_unchecked\(preflight\)[\s\S]*?boundary\.validate_context\(\)\?;\\n"\s*"    Ok\(boundary\.append_boundary_digest\)[\s\S]*?kagemusha_recursive_spend_lineage_append_boundary_digest_unchecked\(boundary\)/u,
    "append digest wrapper bypass negative control must replace checked wrappers with unchecked digest recomputation",
  );
  assert.match(
    wrapperBypassBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append digest wrapper bypass negative control must validate the mutated text snapshot",
  );
  assert.match(
    wrapperBypassBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append digest wrapper bypass drift was not detected"\)/u,
    "append digest wrapper bypass negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    wrapperBypassBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append digest wrapper bypass negative control must not unconditionally pass after run_checks",
  );

  assert.match(
    guard,
    /def check_append_boundary_profile_comparison_is_complete\(\):[\s\S]*?expected_fields = \[[\s\S]*?"domain"[\s\S]*?"append_opening_preflight_digest"[\s\S]*?"resulting_public_inputs_hash"[\s\S]*?"fixed_window_shared_table_manifest_digest"[\s\S]*?"append_boundary_digest"[\s\S]*?actual_fields != expected_fields/u,
    "policy guard must pin the exact append-boundary transition-profile comparison field list",
  );

  const profileComparisonBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-boundary-profile-comparison":'),
    guard.indexOf('if mode == "--negative-control-core-vesta-ipa-h-fold":'),
  );
  assert.match(
    profileComparisonBranch,
    /ensure_field!\(append_boundary_digest\);\\n"[\s\S]*?""/u,
    "append-boundary profile comparison negative control must remove the terminal digest comparison",
  );
  assert.match(
    profileComparisonBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append-boundary profile comparison negative control must validate the mutated text snapshot",
  );
  assert.match(
    profileComparisonBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append-boundary profile comparison drift was not detected"\)/u,
    "append-boundary profile comparison negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    profileComparisonBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append-boundary profile comparison negative control must not unconditionally pass after run_checks",
  );

  assert.match(
    guard,
    /def check_recursive_public_input_schema_order_and_indices\(\):[\s\S]*?expected_recursive_aggregation_public_inputs\(\)[\s\S]*?actual_fields != expected_fields[\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_OPENING_PREFLIGHT_START_INDEX[\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_BOUNDARY_START_INDEX[\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_HOP_COUNT_INDEX/u,
    "policy guard must pin recursive aggregation public-input schema order and core index constants",
  );
  assert.match(
    guard,
    /def expected_recursive_aggregation_instance_value_expressions\(\):[\s\S]*?append_opening_preflight_digest[\s\S]*?append_opening_preflight_limbs[\s\S]*?append_boundary_digest[\s\S]*?append_boundary_limbs[\s\S]*?u64::from\(public_inputs\.hop_count\)[\s\S]*?def check_recursive_public_input_value_builder_order\(\):[\s\S]*?extract_recursive_public_input_value_expressions\(core\)[\s\S]*?actual_values != expected_values[\s\S]*?required_derivations[\s\S]*?function_body/u,
    "policy guard must pin recursive aggregation public-input value builder order",
  );
  assert.match(
    guard,
    /def check_recursive_public_input_non_zero_groups\(\):[\s\S]*?expected_recursive_aggregation_limb_prefixes\(\)\[:9\][\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_NON_ZERO_PUBLIC_FIELD_GROUPS[\s\S]*?actual_groups != expected_groups[\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_NON_ZERO_PUBLIC_FIELD_GROUP_LABELS[\s\S]*?actual_labels != expected_prefixes[\s\S]*?ensure_kagemusha_recursive_compact_token_public_instance_context/u,
    "policy guard must pin recursive aggregation non-zero public field groups and labels",
  );
  assert.match(
    guard,
    /def check_recursive_append_semantic_non_zero_groups\(\):[\s\S]*?validate_append_semantic_profile[\s\S]*?validate_one_hop_semantic_non_zero_witnesses\(semantic\)[\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_TRANSITION_PROFILE_BINDING_START_INDEX[\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_OPENING_PREFLIGHT_START_INDEX[\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_BOUNDARY_START_INDEX[\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_VERIFIER_SCALAR_PROJECTION_START_INDEX[\s\S]*?actual_calls != expected_calls/u,
    "policy guard must pin append verifier-slice semantic non-zero groups",
  );
  assert.match(
    guard,
    /def check_recursive_spend_proof_public_input_circuit_binding\(\):[\s\S]*?expected_kagemusha_recursive_spend_public_inputs_for_proof[\s\S]*?SemanticAggregation[\s\S]*?append_boundary_digest[\s\S]*?Lineage[\s\S]*?recursive_verifier_scalar_projection_digest[\s\S]*?expected\.recursive_verifier_scalar_projection_digest = scalar_projection[\s\S]*?expected\.append_opening_preflight_digest == \[0u8; Hash::LENGTH\][\s\S]*?append_boundary_digest != expected\.append_boundary_digest/u,
    "policy guard must pin recursive spend proof public-input circuit binding",
  );
  assert.match(
    guard,
    /def check_recursive_spend_previous_proof_field_binding\(\):[\s\S]*?ensure_recursive_spend_previous_proof_matches[\s\S]*?"folded_public_inputs_hash"[\s\S]*?"recursive_verifier_scalar_projection_digest"[\s\S]*?actual_fields != expected_fields[\s\S]*?previous_recursive_proof\.public_inputs_hash != expected\.public_inputs_hash\(\)\?/u,
    "policy guard must pin previous recursive proof public-input field binding",
  );

  const schemaOrderBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-recursive-public-input-schema-order":'),
    guard.indexOf('if mode == "--negative-control-core-recursive-public-input-index-map":'),
  );
  assert.match(
    schemaOrderBranch,
    /append_opening_preflight_digest_limb0[\s\S]*?append_boundary_digest_limb0[\s\S]*?append_boundary_digest_limb0[\s\S]*?append_opening_preflight_digest_limb0/u,
    "recursive public-input schema negative control must swap append-opening and append-boundary groups",
  );
  assert.match(
    schemaOrderBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "recursive public-input schema negative control must validate the mutated text snapshot",
  );
  assert.match(
    schemaOrderBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: recursive public-input schema order drift was not detected"\)/u,
    "recursive public-input schema negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    schemaOrderBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "recursive public-input schema negative control must not unconditionally pass after run_checks",
  );

  const indexMapBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-recursive-public-input-index-map":'),
    guard.indexOf('if mode == "--negative-control-core-recursive-public-input-value-order":'),
  );
  assert.match(
    indexMapBranch,
    /KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_BOUNDARY_START_INDEX: usize = 48;[\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_BOUNDARY_START_INDEX: usize = 44;/u,
    "recursive public-input index negative control must shift the append-boundary start index",
  );
  assert.match(
    indexMapBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "recursive public-input index negative control must validate the mutated text snapshot",
  );
  assert.match(
    indexMapBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: recursive public-input index map drift was not detected"\)/u,
    "recursive public-input index negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    indexMapBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "recursive public-input index negative control must not unconditionally pass after run_checks",
  );

  const valueOrderBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-recursive-public-input-value-order":'),
    guard.indexOf('if mode == "--negative-control-core-recursive-public-input-nonzero-groups":'),
  );
  assert.match(
    valueOrderBranch,
    /append_opening_preflight_limbs\[0\][\s\S]*?append_boundary_limbs\[0\][\s\S]*?append_boundary_limbs\[0\][\s\S]*?append_opening_preflight_limbs\[0\]/u,
    "recursive public-input value negative control must swap append-opening and append-boundary value groups",
  );
  assert.match(
    valueOrderBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "recursive public-input value negative control must validate the mutated text snapshot",
  );
  assert.match(
    valueOrderBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: recursive public-input value order drift was not detected"\)/u,
    "recursive public-input value negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    valueOrderBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "recursive public-input value negative control must not unconditionally pass after run_checks",
  );

  const nonzeroGroupBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-recursive-public-input-nonzero-groups":'),
    guard.indexOf('if mode == "--negative-control-core-recursive-append-semantic-nonzero-groups":'),
  );
  assert.match(
    nonzeroGroupBranch,
    /\[32, 33, 34, 35\][\s\S]*?\[28, 29, 30, 31\]/u,
    "recursive public-input nonzero group negative control must drop verifier-witness batch coverage",
  );
  assert.match(
    nonzeroGroupBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "recursive public-input nonzero group negative control must validate the mutated text snapshot",
  );
  assert.match(
    nonzeroGroupBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: recursive public-input nonzero group drift was not detected"\)/u,
    "recursive public-input nonzero group negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    nonzeroGroupBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "recursive public-input nonzero group negative control must not unconditionally pass after run_checks",
  );

  const appendSemanticNonzeroBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-recursive-append-semantic-nonzero-groups":'),
    guard.indexOf('if mode == "--negative-control-core-vesta-ipa-h-fold":'),
  );
  assert.match(
    appendSemanticNonzeroBranch,
    /KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_BOUNDARY_START_INDEX[\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_OPENING_PREFLIGHT_START_INDEX/u,
    "append semantic nonzero group negative control must redirect append-boundary coverage to append-opening",
  );
  assert.match(
    appendSemanticNonzeroBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append semantic nonzero group negative control must validate the mutated text snapshot",
  );
  assert.match(
    appendSemanticNonzeroBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append semantic nonzero group drift was not detected"\)/u,
    "append semantic nonzero group negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    appendSemanticNonzeroBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append semantic nonzero group negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin checked-fold preverification order", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-core-fold-public-input-preverify-order",
    "--negative-control-core-record-backed-fold-public-input-preverify-order",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    expectedModes,
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  for (const mode of expectedModes) {
    assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
    assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);
  }

  assert.match(
    guard,
    /def check_checked_fold_public_input_preverification_order\(\):[\s\S]*?validate_kagemusha_fold_metadata\(steps\)\?;[\s\S]*?validate_required_kagemusha_confidential_v2_step_public_inputs\(chain_id, asset, step\)\?;[\s\S]*?verified_steps\.push\(kagemusha_verified_fold_step\(step\)\?\);[\s\S]*?validate_kagemusha_fold_metadata\(&steps\)\?;[\s\S]*?validate_kagemusha_hop_verifier_record_set\(&steps, records\)\?;[\s\S]*?validate_required_kagemusha_confidential_v2_step_public_inputs\(/u,
    "policy guard must pin direct and record-backed checked-fold public-input preverification order",
  );

  const start = guard.indexOf('if mode == "--negative-control-core-fold-public-input-preverify-order":');
  const end = guard.indexOf('if mode == "--negative-control-core-record-backed-fold-public-input-preverify-order":');
  const branch = guard.slice(start, end);
  assert.match(
    branch,
    /validate_required_kagemusha_confidential_v2_step_public_inputs\(chain_id, asset, step\)\?;\\n"\s*"        verified_steps\.push\(kagemusha_verified_fold_step\(step\)\?\);[\s\S]*?verified_steps\.push\(kagemusha_verified_fold_step\(step\)\?\);\\n"\s*"        validate_required_kagemusha_confidential_v2_step_public_inputs\(chain_id, asset, step\)\?;/u,
    "checked-fold public-input negative control must reorder validation after fold-step recording",
  );
  assert.match(
    branch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "checked-fold public-input negative control must validate the mutated text snapshot",
  );
  assert.match(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: checked-fold public-input preverification order drift was not detected"\)/u,
    "checked-fold public-input negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "checked-fold public-input negative control must not unconditionally pass after run_checks",
  );

  const recordStart = guard.indexOf(
    'if mode == "--negative-control-core-record-backed-fold-public-input-preverify-order":',
  );
  const recordEnd = guard.indexOf('if mode == "--negative-control-core-lineage-witness-fold-predecode":');
  const recordBranch = guard.slice(recordStart, recordEnd);
  assert.match(
    recordBranch,
    /validate_kagemusha_fold_verifier_record\(step, record, block_height\)\?;\\n"[\s\S]*?validate_required_kagemusha_confidential_v2_step_public_inputs\([\s\S]*?validate_required_kagemusha_confidential_v2_step_public_inputs\([\s\S]*?validate_kagemusha_fold_verifier_record\(step, record, block_height\)\?;/u,
    "record-backed checked-fold negative control must reorder verifier-record validation after public-input validation",
  );
  assert.match(
    recordBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "record-backed checked-fold public-input negative control must validate the mutated text snapshot",
  );
  assert.match(
    recordBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: record-backed checked-fold public-input preverification order drift was not detected"\s*\)/u,
    "record-backed checked-fold public-input negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    recordBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "record-backed checked-fold public-input negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin lineage witness preflight coverage", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-core-lineage-witness-fold-predecode",
    "--negative-control-core-lineage-witness-record-predecode",
    "--negative-control-core-lineage-witness-count-mismatch-predecode",
    "--negative-control-core-lineage-witness-envelope-count",
    "--negative-control-core-lineage-witness-malformed-envelope-archive",
    "--negative-control-core-lineage-witness-note-predecode",
    "--negative-control-core-lineage-witness-note-binding-predecode",
    "--negative-control-core-lineage-witness-current-note-invariants",
    "--negative-control-core-lineage-witness-handoff-predecode",
    "--negative-control-core-lineage-witness-duplicate-current-note",
    "--negative-control-core-lineage-witness-final-bundle-context",
    "--negative-control-core-lineage-witness-final-bundle-predecode",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    expectedModes,
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  for (const mode of expectedModes) {
    assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
    assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);
  }

  const policyBranch = (mode) => {
    const start = guard.indexOf(`if mode == "${mode}":`);
    assert.notEqual(start, -1, `missing policy branch ${mode}`);
    const end = guard.indexOf("\nif mode ==", start + 1);
    return guard.slice(start, end === -1 ? guard.length : end);
  };
  const branchSpecs = [
    [
      "--negative-control-core-lineage-witness-fold-predecode",
      /lineage witness root-continuity error should come before Pallas archive decoding[\s\S]*?lineage witness root-continuity error may decode Pallas first/u,
      "lineage witness fold metadata predecode",
    ],
    [
      "--negative-control-core-lineage-witness-record-predecode",
      /lineage witness verifier-record error should come before Pallas archive decoding[\s\S]*?lineage witness verifier-record error may decode Pallas first/u,
      "lineage witness verifier-record predecode",
    ],
    [
      "--negative-control-core-lineage-witness-count-mismatch-predecode",
      /current-note count mismatch: expected 2, found 1[\s\S]*?current-note count mismatch: expected 2, found 0/u,
      "lineage witness count-mismatch predecode",
    ],
    [
      "--negative-control-core-lineage-witness-envelope-count",
      /lineage envelope count mismatch: expected 2, found 0[\s\S]*?lineage envelope count mismatch: expected 2, found 1/u,
      "lineage witness envelope-count",
    ],
    [
      "--negative-control-core-lineage-witness-malformed-envelope-archive",
      /kagemusha_recursive_spend_lineage_witness_rejects_malformed_envelope_archive[\s\S]*?kagemusha_recursive_spend_lineage_witness_allows_malformed_envelope_archive/u,
      "lineage witness malformed envelope archive",
    ],
    [
      "--negative-control-core-lineage-witness-note-predecode",
      /lineage witness current-note error should come before Pallas archive decoding[\s\S]*?lineage witness current-note error may decode Pallas first/u,
      "lineage witness current-note predecode",
    ],
    [
      "--negative-control-core-lineage-witness-note-binding-predecode",
      /lineage witness current-note binding error should come before Pallas archive decoding[\s\S]*?lineage witness current-note binding error may decode Pallas first/u,
      "lineage witness current-note binding predecode",
    ],
    [
      "--negative-control-core-lineage-witness-current-note-invariants",
      /current note 0 spend nullifier must be non-zero[\s\S]*?current note 0 spend nullifier may be zero/u,
      "lineage witness current-note invariants",
    ],
    [
      "--negative-control-core-lineage-witness-handoff-predecode",
      /lineage witness append-handoff error should come before Pallas archive decoding[\s\S]*?lineage witness append-handoff error may decode Pallas first/u,
      "lineage witness append-handoff predecode",
    ],
    [
      "--negative-control-core-lineage-witness-duplicate-current-note",
      /current note 2 spend nullifier is duplicated[\s\S]*?current note 2 spend nullifier may be duplicated/u,
      "lineage witness duplicate current-note spend-nullifier",
    ],
    [
      "--negative-control-core-lineage-witness-final-bundle-context",
      /hop count 1 does not match redeem bundle hop count 2[\s\S]*?hop count 1 may mismatch redeem bundle hop count 2/u,
      "lineage witness final-bundle context",
    ],
    [
      "--negative-control-core-lineage-witness-final-bundle-predecode",
      /lineage witness final-bundle error should come before Pallas archive decoding[\s\S]*?lineage witness final-bundle error may decode Pallas first/u,
      "lineage witness final-bundle predecode",
    ],
  ];

  for (const [mode, mutationPattern, label] of branchSpecs) {
    const branch = policyBranch(mode);
    assert.match(branch, mutationPattern, `${label} negative control must mutate the guarded source text`);
    assert.match(
      branch,
      /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
      `${label} negative control must validate the mutated text snapshot`,
    );
    assert.match(
      branch,
      /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed:/u,
      `${label} negative control must only pass after detecting injected drift`,
    );
    assert.doesNotMatch(
      branch,
      /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
      `${label} negative control must not unconditionally pass after run_checks`,
    );
  }
});

test("recursive Kagemusha policy negative controls pin ABI-7 compact adversarial coverage", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-core-recursive-compact-public-instance-shape",
    "--negative-control-core-recursive-compact-pallas-count",
    "--negative-control-core-recursive-compact-pallas-metadata",
    "--negative-control-core-recursive-compact-cid-spoof-key",
    "--negative-control-core-recursive-spend-compact-projection-token",
    "--negative-control-bridge-recursive-compact-public-instance-shape",
    "--negative-control-bridge-recursive-compact-pallas-count",
    "--negative-control-bridge-recursive-compact-pallas-metadata",
    "--negative-control-bridge-recursive-compact-vk-hash",
    "--negative-control-js-host-recursive-compact-vk-hash",
    "--negative-control-js-host-recursive-compact-pallas-count",
    "--negative-control-js-host-recursive-compact-pallas-metadata",
    "--negative-control-js-host-recursive-compact-public-instance-shape",
    "--negative-control-python-recursive-compact-vk-hash",
    "--negative-control-python-recursive-compact-pallas-count",
    "--negative-control-python-recursive-compact-pallas-metadata",
    "--negative-control-python-recursive-compact-public-instance-shape",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    expectedModes,
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  for (const mode of expectedModes) {
    assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
    assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);
  }

  const policyBranch = (mode) => {
    const start = guard.indexOf(`if mode == "${mode}":`);
    assert.notEqual(start, -1, `missing policy branch ${mode}`);
    const end = guard.indexOf("\nif mode ==", start + 1);
    return guard.slice(start, end === -1 ? guard.length : end);
  };
  const branchSpecs = [
    [
      "--negative-control-core-recursive-compact-public-instance-shape",
      /recursive compact token multi-row public instances must reject[\s\S]*?recursive compact token multi-row public instances may pass/u,
      "core recursive compact public-instance shape",
    ],
    [
      "--negative-control-core-recursive-compact-pallas-count",
      /detached compact Pallas archive must reject before proving[\s\S]*?detached compact Pallas archive may return unavailable[\s\S]*?height-aware detached compact Pallas archive must reject before proving[\s\S]*?height-aware detached compact Pallas archive may return unavailable[\s\S]*?extra compact Pallas opening must reject before proving[\s\S]*?extra compact Pallas opening may return unavailable[\s\S]*?height-aware extra compact Pallas opening must reject before proving[\s\S]*?height-aware extra compact Pallas opening may return unavailable[\s\S]*?missing compact Pallas opening must reject before proving[\s\S]*?missing compact Pallas opening may return unavailable[\s\S]*?height-aware missing compact Pallas opening must reject before proving[\s\S]*?height-aware missing compact Pallas opening may return unavailable[\s\S]*?duplicated multi-hop compact Pallas archive must reject before proving[\s\S]*?duplicated multi-hop compact Pallas archive may return unavailable[\s\S]*?height-aware duplicated multi-hop compact Pallas archive must reject before proving[\s\S]*?height-aware duplicated multi-hop compact Pallas archive may return unavailable[\s\S]*?reordered multi-hop compact Pallas archive must reject before proving[\s\S]*?reordered multi-hop compact Pallas archive may return unavailable[\s\S]*?height-aware reordered multi-hop compact Pallas archive must reject before proving[\s\S]*?height-aware reordered multi-hop compact Pallas archive may return unavailable/u,
      "core recursive compact Pallas opening count",
    ],
    [
      "--negative-control-core-recursive-compact-pallas-metadata",
      /forged multi-hop compact Pallas metadata must reject before proving[\s\S]*?forged multi-hop compact Pallas metadata may return unavailable[\s\S]*?height-aware forged multi-hop compact Pallas metadata must reject before proving[\s\S]*?height-aware forged multi-hop compact Pallas metadata may return unavailable/u,
      "core recursive compact Pallas metadata",
    ],
    [
      "--negative-control-core-recursive-compact-cid-spoof-key",
      /\.expect_err\("CID-spoofed ABI-7 compact verifier key must reject"\);[\s\S]*?\.expect_err\("CID-spoofed ABI-7 compact verifier key may pass"\);[\s\S]*?\.expect_err\("public CID-spoofed ABI-7 compact verifier key must reject"\);[\s\S]*?\.expect_err\("public CID-spoofed ABI-7 compact verifier key may pass"\);/u,
      "core recursive compact CID-spoof key",
    ],
    [
      "--negative-control-core-recursive-spend-compact-projection-token",
      /pub fn verify_kagemusha_recursive_spend_compact_payment_token_projection\([\s\S]*?pub fn verify_kagemusha_recursive_spend_compact_payment_token_projection_unchecked\(/u,
      "core recursive spend compact projection token",
    ],
    [
      "--negative-control-bridge-recursive-compact-public-instance-shape",
      /ABI-7 compact verifier must reject multi-row public instances before returning a soft invalid result[\s\S]*?ABI-7 compact verifier may soft-invalid multi-row public instances/u,
      "bridge recursive compact public-instance shape",
    ],
    [
      "--negative-control-bridge-recursive-compact-pallas-count",
      /ABI-7 compact prover must reject extra valid Pallas opening archives before proving[\s\S]*?ABI-7 compact prover may accept extra valid Pallas opening archives[\s\S]*?ABI-7 compact prover must reject missing valid Pallas opening archives before proving[\s\S]*?ABI-7 compact prover may accept missing valid Pallas opening archives[\s\S]*?ABI-7 compact prover must reject duplicated multi-hop valid Pallas opening archives before proving[\s\S]*?ABI-7 compact prover may accept duplicated multi-hop valid Pallas opening archives[\s\S]*?ABI-7 compact prover must reject reordered valid Pallas opening archives before proving[\s\S]*?ABI-7 compact prover may accept reordered valid Pallas opening archives/u,
      "bridge recursive compact Pallas opening count",
    ],
    [
      "--negative-control-bridge-recursive-compact-pallas-metadata",
      /ABI-7 compact prover must reject forged multi-hop Pallas metadata before proving[\s\S]*?ABI-7 compact prover may accept forged multi-hop Pallas metadata/u,
      "bridge recursive compact Pallas metadata",
    ],
    [
      "--negative-control-bridge-recursive-compact-vk-hash",
      /ABI-7 compact verifier must reject non-canonical envelope verifier-key hashes before returning a soft invalid result[\s\S]*?ABI-7 compact verifier may soft-invalid non-canonical envelope verifier-key hashes/u,
      "bridge recursive compact verifier-key hash",
    ],
    [
      "--negative-control-js-host-recursive-compact-vk-hash",
      /recursive compact token with forged verifier-key hash must reject[\s\S]*?recursive compact token with forged verifier-key hash may soft-invalid/u,
      "JS host recursive compact verifier-key hash",
    ],
    [
      "--negative-control-js-host-recursive-compact-pallas-count",
      /recursive compact prover must reject extra valid Pallas opening archive[\s\S]*?recursive compact prover may accept extra valid Pallas opening archive[\s\S]*?recursive compact prover must reject missing valid Pallas opening archive[\s\S]*?recursive compact prover may accept missing valid Pallas opening archive[\s\S]*?recursive compact prover must reject duplicated multi-hop valid Pallas opening archive[\s\S]*?recursive compact prover may accept duplicated multi-hop valid Pallas opening archive[\s\S]*?recursive compact prover must reject reordered valid Pallas opening archive[\s\S]*?recursive compact prover may accept reordered valid Pallas opening archive/u,
      "JS host recursive compact Pallas opening count",
    ],
    [
      "--negative-control-js-host-recursive-compact-pallas-metadata",
      /recursive compact prover must reject forged multi-hop Pallas metadata[\s\S]*?recursive compact prover may accept forged multi-hop Pallas metadata/u,
      "JS host recursive compact Pallas metadata",
    ],
    [
      "--negative-control-js-host-recursive-compact-public-instance-shape",
      /JS host recursive compact verifier must reject multi-row public instances[\s\S]*?JS host recursive compact verifier may soft-invalid multi-row public instances/u,
      "JS host recursive compact public-instance shape",
    ],
    [
      "--negative-control-python-recursive-compact-vk-hash",
      /recursive compact token with forged verifier-key hash must reject[\s\S]*?recursive compact token with forged verifier-key hash may soft-invalid/u,
      "Python recursive compact verifier-key hash",
    ],
    [
      "--negative-control-python-recursive-compact-pallas-count",
      /recursive compact prover must reject extra valid Pallas opening archive[\s\S]*?recursive compact prover may accept extra valid Pallas opening archive[\s\S]*?recursive compact prover must reject missing valid Pallas opening archive[\s\S]*?recursive compact prover may accept missing valid Pallas opening archive[\s\S]*?recursive compact prover must reject duplicated multi-hop valid Pallas opening archive[\s\S]*?recursive compact prover may accept duplicated multi-hop valid Pallas opening archive[\s\S]*?recursive compact prover must reject reordered valid Pallas opening archive[\s\S]*?recursive compact prover may accept reordered valid Pallas opening archive/u,
      "Python recursive compact Pallas opening count",
    ],
    [
      "--negative-control-python-recursive-compact-pallas-metadata",
      /recursive compact prover must reject forged multi-hop Pallas metadata[\s\S]*?recursive compact prover may accept forged multi-hop Pallas metadata/u,
      "Python recursive compact Pallas metadata",
    ],
    [
      "--negative-control-python-recursive-compact-public-instance-shape",
      /Python recursive compact verifier must reject multi-row public instances[\s\S]*?Python recursive compact verifier may soft-invalid multi-row public instances/u,
      "Python recursive compact public-instance shape",
    ],
  ];

  for (const [mode, mutationPattern, label] of branchSpecs) {
    const branch = policyBranch(mode);
    assert.match(branch, mutationPattern, `${label} negative control must mutate the guarded source text`);
    assert.match(
      branch,
      /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
      `${label} negative control must validate the mutated text snapshot`,
    );
    if (
      mode === "--negative-control-core-recursive-compact-pallas-count"
      || mode === "--negative-control-core-recursive-compact-pallas-metadata"
      || mode === "--negative-control-core-recursive-compact-cid-spoof-key"
      || mode === "--negative-control-bridge-recursive-compact-pallas-count"
      || mode === "--negative-control-js-host-recursive-compact-pallas-count"
      || mode === "--negative-control-python-recursive-compact-pallas-count"
    ) {
      assert.match(
        branch,
        /cases\s*=\s*\([\s\S]*?for before, after, label in cases:[\s\S]*?if label not in message:[\s\S]*?(?:Pallas (?:opening count|metadata)|CID-spoof key) drift was not detected for/u,
        `${label} negative control must check each adversarial case independently`,
      );
      assert.match(
        branch,
        /except\s+PolicyError\s+as\s+error:[\s\S]*?first_message = message[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed:[\s\S]*?raise\s+SystemExit\(0\)/u,
        `${label} negative control must only pass after every adversarial case detects injected drift`,
      );
    } else {
      assert.match(
        branch,
        /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed:/u,
        `${label} negative control must only pass after detecting injected drift`,
      );
    }
    assert.doesNotMatch(
      branch,
      /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
      `${label} negative control must not unconditionally pass after run_checks`,
    );
  }
});

test("recursive Kagemusha policy negative controls pin core Vesta IPA fold coverage", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-core-vesta-ipa-h-fold",
    "--negative-control-core-vesta-ipa-g-fold",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    expectedModes,
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  for (const mode of expectedModes) {
    assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
    assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);
  }

  const hFoldBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-vesta-ipa-h-fold":'),
    guard.indexOf('if mode == "--negative-control-core-vesta-ipa-g-fold":'),
  );
  assert.match(
    hFoldBranch,
    /shared_table_batch_preflight_rejects_h_generator_fold_splice[\s\S]*?shared_table_batch_preflight_allows_h_generator_fold_splice/u,
    "core Vesta IPA H-fold negative control must mutate the shared-table batch H-fold test name",
  );
  assert.match(
    hFoldBranch,
    /accumulator H fold mismatch[\s\S]*?accumulator fold mismatch/u,
    "core Vesta IPA H-fold negative control must remove the H-fold error needle",
  );
  assert.match(
    hFoldBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "core Vesta IPA H-fold negative control must validate the mutated text snapshot",
  );
  assert.match(
    hFoldBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: non-native Vesta IPA H-fold drift was not detected"\)/u,
    "core Vesta IPA H-fold negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    hFoldBranch,
    /\n    raise SystemExit\(0\)\n    raise SystemExit\("negative control failed: non-native Vesta IPA H-fold drift was not detected"\)/u,
    "core Vesta IPA H-fold negative control must not pass after an undetected run_checks result",
  );

  const gFoldBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-vesta-ipa-g-fold":'),
    guard.indexOf('if mode == "--negative-control-data-model-lineage-key-package-binding":'),
  );
  assert.match(
    gFoldBranch,
    /from_pallas_witness_rejects_generator_fold_splice[\s\S]*?from_pallas_witness_allows_generator_fold_splice/u,
    "core Vesta IPA G-fold negative control must mutate the direct G-fold test name",
  );
  assert.match(
    gFoldBranch,
    /accumulator G fold mismatch[\s\S]*?accumulator fold mismatch/u,
    "core Vesta IPA G-fold negative control must remove the G-fold error needle",
  );
  assert.match(
    gFoldBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "core Vesta IPA G-fold negative control must validate the mutated text snapshot",
  );
  assert.match(
    gFoldBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: non-native Vesta IPA G-fold drift was not detected"\)/u,
    "core Vesta IPA G-fold negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    gFoldBranch,
    /\n    raise SystemExit\(0\)\n    raise SystemExit\("negative control failed: non-native Vesta IPA G-fold drift was not detected"\)/u,
    "core Vesta IPA G-fold negative control must not pass after an undetected run_checks result",
  );
});

test("recursive Kagemusha policy negative controls pin data-model lineage key builder exactness", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-data-model-lineage-key-package-binding";

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [mode],
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
  assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);

  const branch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-lineage-key-package-binding":'),
    guard.indexOf('if mode == "--negative-control-core-opening-preflight-splices":'),
  );
  assertContainsAll(
    branch,
    [
      "kagemusha_lineage_key_artifact_packages_reject_profile_splices",
      "KagemushaRecursiveSpendInitRequestV1::new_with_lineage_key_artifacts(",
      "KagemushaRecursiveSpendInitRequestV1::new_with_lineage_key_artifact_package(",
      "KagemushaRecursiveSpendAppendRequestV1::new_with_previous_lineage_proof_witness_and_key_artifacts(",
      "KagemushaRecursiveSpendAppendRequestV1::new_with_previous_lineage_proof_witness_and_key_artifact_package(",
      "let init = init_without_key_artifacts\\n            .with_lineage_key_artifacts(",
      "let init_from_artifact_package = init_without_key_artifacts\\n            .clone()\\n            .with_lineage_key_artifact_package(init_artifacts.clone())",
      "let append = append_without_key_artifacts\\n            .with_lineage_key_artifacts(",
      "let append_from_artifact_package = append_without_key_artifacts\\n            .clone()\\n            .with_lineage_key_artifact_package(append_artifacts.clone())",
    ],
    "data-model lineage key package negative control must pin exact init/append builder call sites",
  );
  assert.match(
    branch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "data-model lineage key package negative control must validate each mutated text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?lineage key package-binding drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: data-model lineage key package-binding drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "data-model lineage key package negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "data-model lineage key package negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin core append-opening preflight declaration", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-core-opening-preflight-splices";

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [mode],
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
  assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);

  const branch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-opening-preflight-splices":'),
    guard.indexOf('if mode == "--negative-control-core-current-hop-opening-metadata-splice":'),
  );
  assert.match(
    branch,
    /pub struct KagemushaRecursiveSpendLineageAppendOpeningPreflight \{[\s\S]*?pub struct KagemushaRecursiveSpendLineageAppendOpeningContext \{/u,
    "append-opening preflight negative control must mutate the exact core preflight declaration",
  );
  assert.match(
    branch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append-opening preflight negative control must validate the mutated text snapshot",
  );
  assert.match(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append opening-preflight splice drift was not detected"\)/u,
    "append-opening preflight negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append-opening preflight negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin core lineage append helper exactness", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-core-lineage-append-helper-exactness";

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [mode],
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
  assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);

  const branch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-lineage-append-helper-exactness":'),
    guard.indexOf('if mode == "--negative-control-core-previous-proof-verifier-context-exactness":'),
  );
  assertContainsAll(
    branch,
    [
      "pub fn kagemusha_recursive_spend_lineage_append_vk_box(",
      "pub fn kagemusha_recursive_spend_lineage_append_vk_box_unchecked(",
      "fn prove_halo2_ipa_kagemusha_recursive_spend_lineage_append_envelope<const LEN: usize>(",
      "fn prove_halo2_ipa_kagemusha_recursive_spend_lineage_append_envelope_unchecked<const LEN: usize>(",
      "for before, after, label in cases:",
      "core lineage append helper exactness drift was not detected for ",
    ],
    "core lineage append helper negative control must pin exact core helper declarations",
  );
  assert.match(
    branch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "core lineage append helper negative control must validate each mutated text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?core lineage append helper exactness drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: core lineage append helper exactness drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "core lineage append helper negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "core lineage append helper negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin core previous-proof verifier-context exactness", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-core-previous-proof-verifier-context-exactness",
    "--negative-control-core-previous-proof-backend-profile",
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    expectedModes,
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  for (const mode of expectedModes) {
    assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
    assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);
  }

  const adversarialCoverage = guard.slice(
    guard.indexOf("ADVERSARIAL_COVERAGE = {"),
    guard.indexOf("SDK_HELPER_EDGE_COVERAGE = {"),
  );
  assertContainsAll(
    adversarialCoverage,
    [
      '"proof.public_inputs.verifier_opening_len = 8;"',
      '"fixed_bytes(b\\"kagemusha-lineage-previous-proof-forged-params\\")"',
      '"fixed_bytes(b\\"kagemusha-lineage-previous-proof-forged-schedule\\")"',
      '"fixed_bytes(b\\"kagemusha-lineage-previous-proof-forged-manifest\\")"',
      '"fixed-window table schedule digest"',
      '"fixed-window shared-table manifest digest"',
    ],
    "core adversarial coverage must pin each previous-proof verifier-context splice fragment",
  );

  const branch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-previous-proof-verifier-context-exactness":'),
    guard.indexOf('if mode == "--negative-control-core-previous-proof-backend-profile":'),
  );
  assertContainsAll(
    branch,
    [
      "proof.public_inputs.verifier_opening_len = 8;",
      "proof.public_inputs.verifier_opening_len = 16;",
      "kagemusha-lineage-previous-proof-forged-params",
      "kagemusha-lineage-previous-proof-params-unchecked",
      "kagemusha-lineage-previous-proof-forged-schedule",
      "kagemusha-lineage-previous-proof-schedule-unchecked",
      "kagemusha-lineage-previous-proof-forged-manifest",
      "kagemusha-lineage-previous-proof-manifest-unchecked",
      "for before, after, label in cases:",
      "core previous-proof verifier-context exactness drift was not detected for ",
    ],
    "core previous-proof verifier-context negative control must pin exact verifier-context mutations",
  );
  assert.match(
    branch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "core previous-proof verifier-context negative control must validate each mutated text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?core previous-proof verifier-context exactness drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: core previous-proof verifier-context exactness drift was not detected"\s*\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "core previous-proof verifier-context negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "core previous-proof verifier-context negative control must not unconditionally pass after run_checks",
  );

  const backendBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-previous-proof-backend-profile":'),
    guard.indexOf('if mode == "--negative-control-core-proof-chain-accumulator":'),
  );
  assert.match(
    backendBranch,
    /previous proof verifier-key backend mismatch must reject[\s\S]*?previous proof verifier-key backend mismatch may pass[\s\S]*?unsupported previous proof circuit id must reject[\s\S]*?unsupported previous proof circuit id may pass/u,
    "core previous-proof backend profile negative control must mutate backend and circuit-id coverage",
  );
  assert.match(
    backendBranch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "core previous-proof backend profile negative control must validate each mutated text snapshot",
  );
  assert.match(
    backendBranch,
    /if label not in message:[\s\S]*?previous-proof backend profile drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: previous-proof backend profile drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "core previous-proof backend profile negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    backendBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "core previous-proof backend profile negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin append verifier-slice exactness", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-core-append-verifier-slice-preflight-binding";

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [mode],
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
  assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);

  const branch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-verifier-slice-preflight-binding":'),
    guard.indexOf('if mode == "--negative-control-core-one-hop-verifier-slice-evidence-binding":'),
  );
  assertContainsAll(
    branch,
    [
      "pub struct KagemushaRecursiveAggregationAppendVerifierSlice<",
      "pub struct KagemushaRecursiveAggregationAppendVerifierSliceUnchecked<",
      "append slice must reject detached current-hop preflight",
      "append slice may accept detached current-hop preflight",
      "fn kagemusha_recursive_aggregation_verifier_scalar_projection_uses_len_dependent_transcript_binding_row",
      "fn kagemusha_recursive_aggregation_verifier_scalar_projection_uses_fixed_transcript_binding_row",
      "one-hop verifier-slice dispatch requires projection side-column inventory",
      "one-hop verifier-slice dispatch may accept scalar-only side columns",
      "one-hop verifier-slice dispatch rejects empty projection side columns",
      "one-hop verifier-slice dispatch may accept empty projection side columns",
      "append verifier-slice dispatch requires projection side-column inventory",
      "append verifier-slice dispatch may accept truncated side columns",
      "append verifier-slice dispatch rejects empty projection side columns",
      "append verifier-slice dispatch may accept empty projection side columns",
      "for before, after, label in cases:",
      "append verifier-slice preflight binding drift was not detected for ",
    ],
    "append verifier-slice negative control must pin exact type and preflight labels",
  );
  assert.match(
    branch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "append verifier-slice negative control must validate each mutated text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?append verifier-slice preflight binding drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: append verifier-slice preflight binding drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "append verifier-slice negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "append verifier-slice negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin Python append-boundary binding exactness", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-python-append-boundary-current-output-set";

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [mode],
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
  assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);

  const branch = guard.slice(
    guard.indexOf('if mode == "--negative-control-python-append-boundary-current-output-set":'),
    guard.indexOf('if mode == "--negative-control-fixed-window-manifest-digest-splice":'),
  );
  assertContainsAll(
    branch,
    [
      "fn kagemusha_recursive_spend_lineage_append_boundary_py(",
      "fn kagemusha_recursive_spend_lineage_append_boundary_python_rejects_duplicate_current_outputs",
      "Python append-boundary helper must reject duplicate current-hop outputs",
      "repeats an output commitment",
    ],
    "Python append-boundary negative control must pin binding, test, and exact error coverage",
  );
  assert.match(
    branch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "Python append-boundary negative control must validate each mutated text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?Python append-boundary current-hop output-set drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: Python append-boundary current-hop output-set drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "Python append-boundary negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Python append-boundary negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin bridge previous-proof opening output clearing", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-bridge-previous-proof-opening-output-clear";

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [mode],
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
  assert.ok(guard.includes(`if mode == "${mode}":`), `policy guard must implement ${mode}`);

  const branch = guard.slice(
    guard.indexOf('if mode == "--negative-control-bridge-previous-proof-opening-output-clear":'),
    guard.indexOf('if mode == "--negative-control-js-host-recursive-compact-vk-hash":'),
  );
  assertContainsAll(
    branch,
    [
      "malformed previous-proof opening archive",
      "empty previous-proof opening vector",
      "over-count previous-proof opening vector",
      'assert!(out_ptr.is_null(), "{case} must not return output bytes");',
    ],
    "bridge previous-proof opening negative control must pin malformed archive cases and output clearing",
  );
  assert.match(
    branch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "bridge previous-proof opening negative control must validate each mutated text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?bridge previous-proof opening output-clear drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: bridge previous-proof opening output-clear drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "bridge previous-proof opening negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "bridge previous-proof opening negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha SDK parity inventories avoid shadowed method names", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_sdk_parity.sh");
  const tupleBody = (name) => {
    const match = guard.match(
      new RegExp(`^${escapeRegExp(name)}\\s*=\\s*\\(\\n(?<body>[\\s\\S]*?)^\\)`, "mu"),
    );
    assert.ok(match?.groups?.body, `SDK parity guard must define ${name}`);
    return match.groups.body;
  };

  const jsLineageExports = tupleBody("REQUIRED_LINEAGE_KEY_ARTIFACT_JS_PUBLIC_EXPORTS");
  assertContainsAll(
    jsLineageExports,
    [
      '"kagemushaRecursiveSpendLineageKeyArtifactsForInit"',
      '"kagemushaRecursiveSpendLineageKeyArtifactsForAppend"',
    ],
    "JS lineage artifact specific export inventory",
  );
  assert.doesNotMatch(
    jsLineageExports,
    /"kagemushaRecursiveSpendLineageKeyArtifacts"/u,
    "JS lineage artifact generic export must stay split from specific exports",
  );
  assert.match(
    tupleBody("REQUIRED_LINEAGE_KEY_ARTIFACT_JS_GENERIC_PUBLIC_EXPORTS"),
    /"kagemushaRecursiveSpendLineageKeyArtifacts"/u,
    "JS lineage artifact generic export inventory must pin the generic helper",
  );
  assert.match(
    guard,
    /REQUIRED_LINEAGE_KEY_ARTIFACT_JS_ALL_PUBLIC_EXPORTS[\s\S]*?REQUIRED_LINEAGE_KEY_ARTIFACT_JS_PUBLIC_EXPORTS[\s\S]*?\+ REQUIRED_LINEAGE_KEY_ARTIFACT_JS_GENERIC_PUBLIC_EXPORTS/u,
    "JS lineage artifact all-export inventory must explicitly recombine split export groups",
  );

  const pythonProjectionMethods = tupleBody(
    "REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_METHODS",
  );
  assert.match(
    pythonProjectionMethods,
    /"kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"/u,
    "Python projection specific inventory must pin the at-height verifier",
  );
  assert.doesNotMatch(
    pythonProjectionMethods,
    /"kagemusha_verify_recursive_spend_compact_payment_token_projection"/u,
    "Python projection base verifier must stay split from the at-height verifier",
  );
  assert.match(
    tupleBody("REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_BASE_METHODS"),
    /"kagemusha_verify_recursive_spend_compact_payment_token_projection"/u,
    "Python projection base inventory must pin the base verifier",
  );
  assert.match(
    guard,
    /REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_ALL_METHODS[\s\S]*?REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_METHODS[\s\S]*?\+ REQUIRED_RECURSIVE_SPEND_COMPACT_PROJECTION_PYTHON_BASE_METHODS/u,
    "Python projection all-method inventory must explicitly recombine split method groups",
  );

  const pythonLineageMethods = tupleBody("REQUIRED_LINEAGE_KEY_ARTIFACT_PYTHON_PUBLIC_METHODS");
  assertContainsAll(
    pythonLineageMethods,
    [
      '"kagemusha_recursive_spend_lineage_key_artifacts_for_init"',
      '"kagemusha_recursive_spend_lineage_key_artifacts_for_append"',
      '"validate_kagemusha_recursive_spend_lineage_key_artifacts"',
    ],
    "Python lineage artifact specific method inventory",
  );
  assert.doesNotMatch(
    pythonLineageMethods,
    /"kagemusha_recursive_spend_lineage_key_artifacts"/u,
    "Python lineage artifact generic method must stay split from specific methods",
  );
  assert.match(
    tupleBody("REQUIRED_LINEAGE_KEY_ARTIFACT_PYTHON_GENERIC_PUBLIC_METHODS"),
    /"kagemusha_recursive_spend_lineage_key_artifacts"/u,
    "Python lineage artifact generic method inventory must pin the generic helper",
  );
  assert.match(
    guard,
    /REQUIRED_LINEAGE_KEY_ARTIFACT_PYTHON_ALL_PUBLIC_METHODS[\s\S]*?REQUIRED_LINEAGE_KEY_ARTIFACT_PYTHON_PUBLIC_METHODS[\s\S]*?\+ REQUIRED_LINEAGE_KEY_ARTIFACT_PYTHON_GENERIC_PUBLIC_METHODS/u,
    "Python lineage artifact all-method inventory must explicitly recombine split method groups",
  );

  const jniMethods = tupleBody("REQUIRED_RECURSIVE_COMPACT_JNI_METHODS");
  assert.match(
    jniMethods,
    /"nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight"/u,
    "JNI projection specific inventory must pin the at-height verifier",
  );
  assert.doesNotMatch(
    jniMethods,
    /"nativeVerifyRecursiveSpendCompactPaymentTokenProjection"/u,
    "JNI projection base verifier must stay split from the at-height verifier",
  );
  assert.match(
    tupleBody("REQUIRED_RECURSIVE_COMPACT_JNI_BASE_METHODS"),
    /"nativeVerifyRecursiveSpendCompactPaymentTokenProjection"/u,
    "JNI projection base inventory must pin the base verifier",
  );
  assert.match(
    guard,
    /REQUIRED_RECURSIVE_COMPACT_JNI_ALL_METHODS[\s\S]*?REQUIRED_RECURSIVE_COMPACT_JNI_METHODS[\s\S]*?\+ REQUIRED_RECURSIVE_COMPACT_JNI_BASE_METHODS/u,
    "JNI projection all-method inventory must explicitly recombine split method groups",
  );
});

test("recursive Kagemusha SDK parity negative controls fail when drift is undetected", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_sdk_parity.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control",
    "--negative-control-workflow",
    "--negative-control-native-manifest-workflow",
    "--negative-control-js-browser-helper",
    "--negative-control-js-lineage-key-artifact-copy",
    "--negative-control-js-lineage-key-package-binding",
    "--negative-control-js-kagemusha-instruction-transaction-builder",
    "--negative-control-js-python-native-output-headers",
    "--negative-control-python-kagemusha-instruction-transaction-builder",
    "--negative-control-csharp-kagemusha-instruction-transaction-builder",
    "--negative-control-python-lineage-key-package-binding",
    "--negative-control-csharp-lineage-key-package-binding",
    "--negative-control-csharp-lineage-cid1-exactness",
    "--negative-control-csharp-canonical-request-exactness",
    "--negative-control-csharp-identifier-receipt-exactness",
    "--negative-control-csharp-transaction-builder-exactness",
    "--negative-control-csharp-transaction-encoding-exactness",
    "--negative-control-csharp-pallas-builder-surface",
    "--negative-control-swift-lineage-key-package-binding",
    "--negative-control-csharp-lineage-witness-availability-probe",
    "--negative-control-csharp-lineage-witness-append-availability-probe",
    "--negative-control-swift-lineage-witness-availability-probe",
    "--negative-control-swift-lineage-witness-append-availability-probe",
    "--negative-control-jvm-lineage-key-package-binding",
    "--negative-control-android-lineage-key-package-binding",
    "--negative-control-jvm-lineage-witness-availability-probe",
    "--negative-control-jvm-lineage-witness-append-availability-probe",
    "--negative-control-android-lineage-witness-availability-probe",
    "--negative-control-android-lineage-witness-append-availability-probe",
    "--negative-control-jvm-pallas-builder-input-guards",
    "--negative-control-non-csharp-pallas-builder-input-guards",
    "--negative-control-non-csharp-pallas-builder-native-output-guards",
    "--negative-control-js-lineage-readonly-declarations",
    "--negative-control-sdk-archive-input-copy",
    "--negative-control-sdk-lineage-proving-key-copy",
    "--negative-control-sdk-helper-surface",
    "--negative-control-sdk-readme-boundary",
    "--negative-control-sdk-readme-proof-chain-accumulator",
    "--negative-control-sdk-readme-pallas-builder-surface",
    "--negative-control-offline-doc-native-owned-accumulator-boundary",
    "--negative-control-offline-doc-pallas-builder-surface",
    "--negative-control-offline-doc-instruction-transaction-surface",
    "--negative-control-sdk-proof-chain-accumulator-input",
    "--negative-control-sdk-accumulator-digest-inputs",
    "--negative-control-sdk-accumulator-boundary-digest-inputs",
    "--negative-control-sdk-readme-availability-surface",
    "--negative-control-sdk-readme-recursive-compact-unavailable",
    "--negative-control-sdk-readme-compact-projection-verifier",
    "--negative-control-sdk-readme-stale-future-lineage",
    "--negative-control-sdk-readme-native-output-csharp",
    "--negative-control-cross-sdk-helper-bodies",
    "--negative-control-cross-sdk-preferred-mode-fallback",
    "--negative-control-mobile-halo2-vk-hash",
    "--negative-control-rust-recursive-compact-unavailable-classifier",
    "--negative-control-sdk-recursive-compact-unavailable-helper",
    "--negative-control-recursive-compact-verifier-surface",
    "--negative-control-recursive-compact-key-package-arity",
    "--negative-control-python-recursive-compact-probe-arity",
    "--negative-control-js-recursive-compact-key-package-dispatch",
    "--negative-control-js-package-dist-recursive-compact-declarations",
    "--negative-control-js-package-dist-accumulator-digest-declarations",
    "--negative-control-js-package-dist-accumulator-digest-denylist",
    "--negative-control-js-package-dist-terminal-accumulator-digest-denylist",
    "--negative-control-js-package-dist-prefixed-accumulator-digest-denylist",
    "--negative-control-js-package-dist-suffixed-accumulator-digest-denylist",
    "--negative-control-js-package-dist-declaration-sweep",
    "--negative-control-js-package-dist-nexus-declaration-sweep",
    "--negative-control-js-package-dist-kotodama-declaration-sweep",
    "--negative-control-js-dts-recursive-compact-key-package",
    "--negative-control-python-recursive-compact-root-export",
    "--negative-control-recursive-spend-compact-projection-surface",
    "--negative-control-js-compact-projection-block-height-validation",
    "--negative-control-python-recursive-spend-compact-projection-root-export",
    "--negative-control-jvm-compact-projection-unsigned-block-height",
    "--negative-control-jvm-claim-identifier-account-binding-test",
    "--negative-control-jvm-claim-identifier-account-exactness",
    "--negative-control-jvm-identifier-claim-record-exactness",
    "--negative-control-js-swift-identifier-claim-record-exactness",
    "--negative-control-ram-lfe-response-exactness",
    "--negative-control-ram-lfe-program-policy-exactness",
    "--negative-control-identifier-policy-proof-verifier-exactness",
    "--negative-control-identifier-policy-metadata-exactness",
    "--negative-control-account-alias-resolution-exactness",
    "--negative-control-multisig-resolved-account-exactness",
    "--negative-control-android-device-lab-family-fail-closed",
    "--negative-control-android-device-lab-family-overmatch",
    "--negative-control-android-device-lab-family-override-binding",
    "--negative-control-android-device-lab-assembler-identity-fields",
    "--negative-control-native-bridge-zero-envelope-pallas-guard",
    "--negative-control-kagemusha-abi-probe-bounds",
    "--negative-control-kagemusha-probe-rejection-shape",
    "--negative-control-sdk-negative-controls-workflow",
    "--negative-control-sdk-negative-controls-comment-workflow",
    "--negative-control-sdk-main-guard-workflow",
    "--negative-control-bytecode-workflow",
    "--negative-control-native-bridge-job-workflow",
    "--negative-control-native-bridge-runner-workflow",
    "--negative-control-native-bridge-cache-workflow",
    "--negative-control-native-bridge-test-workflow",
    "--negative-control-native-bridge-windowed-record-order-workflow",
    "--negative-control-native-bridge-needs-workflow",
    "--negative-control-python-sdk-job-workflow",
    "--negative-control-python-sdk-runner-workflow",
    "--negative-control-python-sdk-setup-workflow",
    "--negative-control-python-sdk-version-workflow",
    "--negative-control-python-sdk-setup-order-workflow",
    "--negative-control-python-sdk-rust-cache-workflow",
    "--negative-control-python-sdk-timeout-workflow",
    "--negative-control-python-sdk-version-script",
    "--negative-control-python-sdk-override-script",
    "--negative-control-python-sdk-resolver-script",
    "--negative-control-python-sdk-major-script",
    "--negative-control-python-sdk-venv-rebuild-script",
    "--negative-control-python-sdk-native-build-script",
    "--negative-control-python-sdk-venv-activation-script",
    "--negative-control-python-sdk-bytecode-script",
    "--negative-control-python-sdk-test-filter-script",
    "--negative-control-python-sdk-abi7-fixture-native-guard",
    "--negative-control-abi7-sdk-manifest-coverage",
    "--negative-control-python-connect-runner-coverage",
    "--negative-control-python-connect-test-exactness",
    "--negative-control-python-sdk-canonical-request-test-filter-script",
    "--negative-control-python-sdk-identifier-receipt-test-filter-script",
    "--negative-control-python-sdk-multisig-response-test-filter-script",
    "--negative-control-identifier-receipt-proof-base64-guard",
    "--negative-control-identifier-receipt-kind-exactness-guard",
    "--negative-control-identifier-receipt-proof-base64-exactness-guard",
    "--negative-control-identifier-receipt-signature-exactness-guard",
    "--negative-control-identifier-receipt-policy-id-exactness-guard",
    "--negative-control-identifier-receipt-policy-summary-id-exactness-guard",
    "--negative-control-identifier-receipt-program-id-exactness-guard",
    "--negative-control-identifier-receipt-account-id-exactness-guard",
    "--negative-control-swift-identifier-receipt-account-id-exactness",
    "--negative-control-native-bridge-identifier-receipt-exactness",
    "--negative-control-native-bridge-claim-identifier-account-binding",
    "--negative-control-identifier-receipt-hash-exactness-guard",
    "--negative-control-identifier-receipt-timestamp-exactness-guard",
    "--negative-control-identifier-receipt-timestamp-u64-guard",
    "--negative-control-identifier-receipt-resolver-key-exactness-guard",
    "--negative-control-python-sdk-event-filter-test-filter-script",
    "--negative-control-python-sdk-workflow-inventory",
    "--negative-control-python-lineage-frozen-copy",
    "--negative-control-python-sdk-test-workflow",
    "--negative-control-python-host-test-workflow",
    "--negative-control-python-sdk-needs-workflow",
    "--negative-control-jvm-sdk-job-workflow",
    "--negative-control-jvm-sdk-runner-workflow",
    "--negative-control-jvm-sdk-java-setup-workflow",
    "--negative-control-jvm-sdk-java-distribution-workflow",
    "--negative-control-jvm-sdk-java-version-workflow",
    "--negative-control-jvm-sdk-test-workflow",
    "--negative-control-jvm-sdk-test-filter-script",
    "--negative-control-jvm-sdk-canonical-request-test-filter-script",
    "--negative-control-jvm-sdk-signing-verifier-test-filter-script",
    "--negative-control-jvm-sdk-torii-event-stream-verifier-filter-script",
    "--negative-control-jvm-sdk-identifier-receipt-filter-script",
    "--negative-control-jvm-sdk-workflow-inventory",
    "--negative-control-jvm-sdk-android-workflow-inventory",
    "--negative-control-jvm-sdk-jdk21-script",
    "--negative-control-jvm-sdk-java-home-override-script",
    "--negative-control-jvm-sdk-java-home-reject-script",
    "--negative-control-jvm-recursive-compact-verifier-availability",
    "--negative-control-jvm-recursive-compact-shape-classifier",
    "--negative-control-mobile-recursive-spend-native-output-headers",
    "--negative-control-mobile-privacy-production-gate-exactness",
    "--negative-control-mobile-privacy-audit-hash-uniqueness",
    "--negative-control-mobile-privacy-localnet-lifecycle-audit",
    "--negative-control-public-privacy-localnet-lifecycle-catalog",
    "--negative-control-public-privacy-sdk-export-review-scope-evidence",
    "--negative-control-public-privacy-zero-hash-evidence",
    "--negative-control-public-privacy-repeated-hash-evidence",
    "--negative-control-public-privacy-zero-signature-evidence",
    "--negative-control-public-privacy-repeated-signature-evidence",
    "--negative-control-public-privacy-reviewer-identity-evidence",
    "--negative-control-public-privacy-artifact-label-evidence",
    "--negative-control-public-privacy-duplicate-row-evidence",
    "--negative-control-public-privacy-deterministic-test-artifact",
    "--negative-control-mobile-zk-merkle-provider-adversarial-coverage",
    "--negative-control-mobile-zk-torii-parser-shape-coverage",
    "--negative-control-mobile-confidential-note-coverage",
    "--negative-control-mobile-offline-readiness-coverage",
    "--negative-control-kotlin-offline-cash-settlement-coverage",
    "--negative-control-android-offline-transfer-persistence-coverage",
    "--negative-control-mobile-transaction-norito-runner-coverage",
    "--negative-control-kotlin-norito-framing-runner-coverage",
    "--negative-control-mobile-account-address-canonical-coverage",
    "--negative-control-mobile-connect-runner-coverage",
    "--negative-control-mobile-transport-inspector-attestation-coverage",
    "--negative-control-mobile-sccp-runner-coverage",
    "--negative-control-mobile-torii-rpc-subscription-websocket-runner-coverage",
    "--negative-control-jvm-offline-note-v2-decoder-placeholder",
    "--negative-control-jvm-offline-note-v2-instruction-wrapper",
    "--negative-control-jvm-offline-note-v2-instruction-decoder",
    "--negative-control-offline-note-v2-canonical-instruction-wire-names",
    "--negative-control-swift-offline-note-v2-decoder-placeholder",
    "--negative-control-swift-offline-note-v2-instruction-decoder",
    "--negative-control-jvm-sdk-android-harness-script",
    "--negative-control-jvm-sdk-test-order-workflow",
    "--negative-control-jvm-sdk-needs-workflow",
    "--negative-control-swift-sdk-job-workflow",
    "--negative-control-swift-sdk-runner-workflow",
    "--negative-control-swift-sdk-parse-workflow",
    "--negative-control-swift-sdk-parse-surface-script",
    "--negative-control-swift-connect-parse-surface-script",
    "--negative-control-swift-sdk-privacy-parse-script",
    "--negative-control-swift-sdk-torii-verifier-parse-script",
    "--negative-control-swift-sdk-workflow-inventory",
    "--negative-control-swift-sdk-source-workflow-inventory",
    "--negative-control-swift-sdk-uc4-skip",
    "--negative-control-swift-lineage-data-copy",
    "--negative-control-swift-recursive-compact-verifier-bool",
    "--negative-control-swift-recursive-compact-verifier-availability",
    "--negative-control-swift-kagemusha-native-output-cap",
    "--negative-control-swift-native-output-headers",
    "--negative-control-swift-native-input-headers",
    "--negative-control-swift-kagemusha-instruction-transaction-builder",
    "--negative-control-swift-identifier-receipt-account-id-decode-test",
    "--negative-control-swift-nfc-receive-success-preservation",
    "--negative-control-swift-nfc-receipt-ack-single-success",
    "--negative-control-swift-nfc-receipt-ack-read-single-success",
    "--negative-control-swift-nfc-emulation-progress-after-success",
    "--negative-control-swift-nfc-send-terminal-success-policy",
    "--negative-control-swift-sdk-version-script",
    "--negative-control-swift-sdk-override-script",
    "--negative-control-swift-sdk-needs-workflow",
    "--negative-control-csharp-sdk-job-workflow",
    "--negative-control-csharp-sdk-setup-workflow",
    "--negative-control-csharp-sdk-dotnet-version-workflow",
    "--negative-control-csharp-sdk-setup-order-workflow",
    "--negative-control-csharp-sdk-dotnet-version-script",
    "--negative-control-csharp-sdk-dotnet-info-script",
    "--negative-control-csharp-sdk-dotnet-override-script",
    "--negative-control-csharp-sdk-dotnet-major-script",
    "--negative-control-csharp-sdk-native-bridge-script",
    "--negative-control-csharp-sdk-native-library-evidence-script",
    "--negative-control-csharp-sdk-test-filter-script",
    "--negative-control-csharp-sdk-verifier-backend-test-filter-script",
    "--negative-control-csharp-sdk-workflow-inventory",
    "--negative-control-csharp-archive-copy",
    "--negative-control-csharp-recursive-compact-verifier-unavailable",
    "--negative-control-csharp-sdk-test-workflow",
    "--negative-control-csharp-sdk-needs-workflow",
    "--negative-control-js-sdk-job-workflow",
    "--negative-control-js-sdk-runner-workflow",
    "--negative-control-js-sdk-node-setup-workflow",
    "--negative-control-js-sdk-node-version-workflow",
    "--negative-control-js-sdk-node-version-script",
    "--negative-control-js-sdk-node-override-script",
    "--negative-control-js-sdk-node-resolver-script",
    "--negative-control-js-sdk-node-major-script",
    "--negative-control-js-sdk-node-cache-workflow",
    "--negative-control-js-sdk-node-setup-order-workflow",
    "--negative-control-js-sdk-install-workflow",
    "--negative-control-js-sdk-native-build-workflow",
    "--negative-control-js-sdk-test-workflow",
    "--negative-control-js-sdk-transaction-builder-filter-script",
    "--negative-control-js-sdk-privacy-native-filter-script",
    "--negative-control-js-sdk-offline-cash-filter-script",
    "--negative-control-js-sdk-canonical-request-filter-script",
    "--negative-control-js-sdk-event-filter-filter-script",
    "--negative-control-js-sdk-verifier-key-filter-script",
    "--negative-control-js-sdk-identifier-receipt-filter-script",
    "--negative-control-js-torii-runner-coverage",
    "--negative-control-js-connect-runner-coverage",
    "--negative-control-js-sdk-workflow-inventory",
    "--negative-control-sdk-privacy-workflow-inventory-matrix",
    "--negative-control-js-sdk-install-order-workflow",
    "--negative-control-js-sdk-test-order-workflow",
    "--negative-control-js-sdk-native-build-order-workflow",
    "--negative-control-js-sdk-needs-workflow",
    "--negative-control-sdk-parity-meta-test-workflow",
    "--negative-control-sdk-negative-controls-order-workflow",
  ];
  assert.match(
    workflow,
    /"ci\/check_no_tracked_python_bytecode\.sh"/,
    "Kagemusha workflow must trigger on the tracked Python bytecode guard",
  );
  assert.match(
    workflow,
    /Reject tracked Python bytecode[\s\S]*run:\s+bash ci\/check_no_tracked_python_bytecode\.sh[\s\S]*Kagemusha recursive spend SDK parity negative controls/,
    "Kagemusha workflow must reject tracked Python bytecode before SDK parity negative controls",
  );
  assert.match(
    workflow,
    /Reject tracked Python bytecode[\s\S]*run:\s+bash ci\/check_no_tracked_python_bytecode\.sh[\s\S]*- name:\s+Kagemusha recursive spend SDK parity\s*\n\s*run:\s+ci\/check_kagemusha_recursive_spend_sdk_parity\.sh/,
    "Kagemusha workflow must reject tracked Python bytecode before SDK parity",
  );
  assert.match(
    guard,
    /testIdentifierReceiptRejectsMalformedProofAttestationBase64DuringDecode[\s\S]*testIdentifierReceiptDecodeRejectsPaddedAccountIdBeforeSignatureVerification[\s\S]*"account_id"[\s\S]*Swift identifier receipt account-id, attestation kind, and malformed proof base64 tests/u,
    "Kagemusha SDK parity guard must pin the Swift padded account-id receipt decode regression",
  );
  assertWorkflowIncludesPaths(
    workflow,
    [
      ...quotedStringsFromInventory(guard, "SOURCE_PATHS = (", "NATIVE_MANIFEST_PATHS = ("),
      ...quotedStringsFromInventory(guard, "NATIVE_MANIFEST_PATHS = (", "SDK_README_PATHS = ("),
      ...quotedStringsFromInventory(guard, "SDK_README_PATHS = (", "WORKFLOW_PATH ="),
      quotedConstant(guard, "WORKFLOW_PATH"),
      quotedConstant(guard, "JS_PARITY_TEST_PATH"),
      ...quotedStringsFromInventory(
        guard,
        "WORKFLOW_REQUIRED_PATHS = SOURCE_PATHS + (",
        "SDK_PARITY_MAIN_COMMAND =",
      ),
    ],
    "Kagemusha SDK parity guard",
  );
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_sdk_parity.sh",
    negativeControlModesFromInventory(
      guard,
      "SDK_PARITY_NEGATIVE_CONTROL_COMMANDS = (",
      "class ParityError",
    ),
    "Kagemusha SDK parity guard",
  );
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_connect_norito_bridge_header.sh",
    REQUIRED_HEADER_NEGATIVE_CONTROL_MODES,
    "NoritoBridge header guard",
  );
  for (const mode of expectedModes) {
    assert.ok(
      guard.includes(`if mode == "${mode}":`),
      `SDK parity guard must implement ${mode}`,
    );
    assert.match(
      workflow,
      new RegExp(
        `^\\s+ci/check_kagemusha_recursive_spend_sdk_parity\\.sh ${mode}$`,
        "m",
      ),
      `Kagemusha workflow must run SDK parity ${mode}`,
    );
  }
  const swiftIdentifierAccountIdExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-identifier-receipt-account-id-exactness":'),
    guard.indexOf('if mode == "--negative-control-identifier-receipt-hash-exactness-guard":'),
  );
  assert.match(
    swiftIdentifierAccountIdExactnessBranch,
    /accountId = rawAccountId[\s\S]*?accountId = trimmedAccountId/u,
    "Swift identifier receipt account-id exactness negative control must reintroduce whitespace normalization",
  );
  assert.match(
    swiftIdentifierAccountIdExactnessBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift identifier receipt account-id exactness negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftIdentifierAccountIdExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift identifier receipt account-id exactness drift was not detected"\)/u,
    "Swift identifier receipt account-id exactness negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftIdentifierAccountIdExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift identifier receipt account-id exactness negative control must not unconditionally pass after run_checks",
  );
  const nativeBridgeIdentifierReceiptExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-native-bridge-identifier-receipt-exactness":'),
    guard.indexOf('if mode == "--negative-control-native-bridge-claim-identifier-account-binding":'),
  );
  assert.match(
    nativeBridgeIdentifierReceiptExactnessBranch,
    /raw\.is_empty\(\) \|\| raw\.trim\(\) != raw[\s\S]*?raw\.trim\(\)\.is_empty/u,
    "native bridge identifier receipt exactness negative control must reintroduce whitespace normalization",
  );
  assert.match(
    nativeBridgeIdentifierReceiptExactnessBranch,
    /parse_identifier_receipt_rejects_padded_payload_fields[\s\S]*?parse_identifier_receipt_allows_padded_payload_fields/u,
    "native bridge identifier receipt exactness negative control must mutate padded payload coverage",
  );
  assert.match(
    guard,
    /let kind =\\n\s+parse_identifier_exact_str[\s\S]*?vec!\["payload", "opaque_id"\][\s\S]*?vec!\["payload", "uaid"\][\s\S]*?vec!\["payload", "account_id"\][\s\S]*?vec!\["attestation", "kind"\][\s\S]*?Rust native identifier receipt exactness/u,
    "native bridge identifier receipt exactness guard must pin padded opaque_id, uaid, account_id, and attestation kind coverage",
  );
  assert.match(
    nativeBridgeIdentifierReceiptExactnessBranch,
    /Rust native identifier receipt exactness[\s\S]*?native bridge identifier receipt exactness drift was not detected/u,
    "native bridge identifier receipt exactness negative control must only pass after detecting injected drift",
  );
  assert.match(
    guard,
    /fn validate_identifier_claim_account\([\s\S]*?if &receipt\.payload\.account_id != account[\s\S]*?validate_identifier_claim_account\(&account, &receipt\)\?;[\s\S]*?validate_identifier_claim_account_rejects_mismatched_receipt_account[\s\S]*?Rust native claim identifier account binding/u,
    "native bridge claim-identifier guard must pin account/receipt account binding before transaction encoding",
  );
  const nativeBridgeClaimIdentifierAccountBindingBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-native-bridge-claim-identifier-account-binding":'),
    guard.indexOf('if mode == "--negative-control-identifier-receipt-hash-exactness-guard":'),
  );
  assert.match(
    nativeBridgeClaimIdentifierAccountBindingBranch,
    /validate_identifier_claim_account\(&account, &receipt\)\?;\\n[\s\S]*?validate_identifier_claim_account_rejects_mismatched_receipt_account[\s\S]*?validate_identifier_claim_account_allows_mismatched_receipt_account/u,
    "native bridge claim-identifier negative control must remove account binding and mutate its regression test",
  );
  assert.match(
    nativeBridgeClaimIdentifierAccountBindingBranch,
    /Rust native claim identifier account binding[\s\S]*?native claim-identifier account binding drift was not detected/u,
    "native bridge claim-identifier negative control must only pass after detecting injected drift",
  );
  assert.match(
    guard,
    /claimIdentifierRejectsAccountMismatchBeforeEncoding[\s\S]*?claim identifier rejects account mismatch before encoding[\s\S]*?ClaimIdentifier accountId must match receipt\.accountId[\s\S]*?ClaimIdentifier account binding test/u,
    "JVM ClaimIdentifier guard must pin Java and Kotlin account-mismatch regression tests",
  );
  assertContainsAll(
    guard,
    [
      "Python identifier receipt resolver-key parser exactness",
      'r"def _identifier_decode_public_key\\(value: Any, context: str\\) -> Tuple\\[str, bytes\\]:[\\s\\S]*?"',
      'r"literal = _require_exact_non_empty_string\\(value, context\\)[\\s\\S]*?"',
      'r\'prefix, multihash_literal = literal\\.split\\(":", 1\\)[\\s\\S]*?\'',
      'r"prefixed_algorithm = prefix\\.lower\\(\\)[\\s\\S]*?"',
      'r"if prefixed_algorithm and prefixed_algorithm != algorithm"',
    ],
    "Python identifier receipt resolver-key guard must pin the exact resolver-key parser block",
  );
  const identifierResolverKeyExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-identifier-receipt-resolver-key-exactness-guard":'),
    guard.indexOf('if mode == "--negative-control-python-sdk-event-filter-test-filter-script":'),
  );
  assert.match(
    identifierResolverKeyExactnessBranch,
    /literal = _require_exact_non_empty_string\(value, context\)[\s\S]*?literal = _require_non_empty_string\(value, context\)\.strip\(\)[\s\S]*?prefixed_algorithm = prefix\.lower\(\)[\s\S]*?prefixed_algorithm = prefix\.strip\(\)\.lower\(\)/u,
    "Python identifier receipt resolver-key negative control must mutate exact literal and prefix parsing independently",
  );
  assert.match(
    identifierResolverKeyExactnessBranch,
    /cases\s*=\s*\([\s\S]*?for before, after, label in cases:[\s\S]*?if label not in message and "Python identifier receipt resolver-key parser exactness" not in message/u,
    "Python identifier receipt resolver-key negative control must check each resolver-key parser case independently",
  );
  assert.match(
    identifierResolverKeyExactnessBranch,
    /finally:[\s\S]*?text_overrides\.pop\(target, None\)[\s\S]*?texts\[target\] = original[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: identifier receipt resolver-key exactness drift was not detected"\s*\)/u,
    "Python identifier receipt resolver-key negative control must reset mutated snapshots and only pass after detected drift",
  );
  const jvmClaimIdentifierAccountBindingBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-jvm-claim-identifier-account-binding-test":'),
    guard.indexOf('if mode == "--negative-control-jvm-claim-identifier-account-exactness":'),
  );
  assert.match(
    jvmClaimIdentifierAccountBindingBranch,
    /claimIdentifierRejectsAccountMismatchBeforeEncoding[\s\S]*?claimIdentifierAllowsAccountMismatchBeforeEncoding[\s\S]*?claim identifier rejects account mismatch before encoding[\s\S]*?claim identifier allows account mismatch before encoding/u,
    "JVM ClaimIdentifier negative control must mutate Java and Kotlin regression test names",
  );
  assert.match(
    jvmClaimIdentifierAccountBindingBranch,
    /ClaimIdentifier accountId must match receipt\.accountId[\s\S]*?ClaimIdentifier account mismatch may be checked by core/u,
    "JVM ClaimIdentifier negative control must remove the explicit mismatch assertion marker",
  );
  assert.match(
    jvmClaimIdentifierAccountBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?JVM ClaimIdentifier account binding drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JVM ClaimIdentifier account binding drift was not detected"\)/u,
    "JVM ClaimIdentifier negative control must only pass after detecting injected drift",
  );
  assert.match(
    guard,
    /Android Java ClaimIdentifier account exactness[\s\S]*?Kotlin ClaimIdentifier account exactness[\s\S]*?requireExactNonBlank\(accountId, \\"accountId\\"\)[\s\S]*?requireExactNonBlank\(receipt\.accountId[\s\S]*?padded ClaimIdentifier account must fail before encoding[\s\S]*?accountId must not contain surrounding whitespace/u,
    "JVM ClaimIdentifier guard must pin exact account text and padded-account rejection",
  );
  const jvmClaimIdentifierAccountExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-jvm-claim-identifier-account-exactness":'),
    guard.indexOf('if mode == "--negative-control-jvm-identifier-claim-record-exactness":'),
  );
  assert.match(
    jvmClaimIdentifierAccountExactnessBranch,
    /requireExactNonBlank\(accountId, \\"accountId\\"\)[\s\S]*?requireNonBlank\(accountId, \\"accountId\\"\)[\s\S]*?requireExactNonBlank\(receipt\.accountId[\s\S]*?requireNonBlank\(receipt\.accountId/u,
    "JVM ClaimIdentifier account exactness negative control must mutate Java and Kotlin production exact helpers",
  );
  assert.match(
    jvmClaimIdentifierAccountExactnessBranch,
    /padded ClaimIdentifier account must fail before encoding[\s\S]*?padded ClaimIdentifier account may normalize before encoding[\s\S]*?accountId must not contain surrounding whitespace[\s\S]*?accountId may be trimmed before encoding/u,
    "JVM ClaimIdentifier account exactness negative control must mutate padded-account regression markers",
  );
  assert.match(
    jvmClaimIdentifierAccountExactnessBranch,
    /Android Java ClaimIdentifier account exactness[\s\S]*?Kotlin ClaimIdentifier account exactness[\s\S]*?JVM ClaimIdentifier account exactness drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JVM ClaimIdentifier account exactness drift was not detected"\)/u,
    "JVM ClaimIdentifier account exactness negative control must only pass after detecting injected drift",
  );
  assert.match(
    guard,
    /requiredExactString\(root\.get\("policy_id"\), "identifier claim record\.policy_id"\)[\s\S]*?Android Java identifier claim record policy-id exactness[\s\S]*?requiredExactString\(root\["policy_id"\], "identifier claim record\.policy_id"\)[\s\S]*?Kotlin identifier claim record policy-id exactness[\s\S]*?identifierClaimRecordParserRejectsNonExactClaimFields[\s\S]*?identifier claim record parser must reject non-exact[\s\S]*?identifier claim record \$label exactness/u,
    "JVM identifier claim-record guard must pin exact parser fields and Java/Kotlin padded-field tests",
  );
  const jvmIdentifierClaimRecordExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-jvm-identifier-claim-record-exactness":'),
    guard.indexOf('if mode == "--negative-control-js-swift-identifier-claim-record-exactness":'),
  );
  assert.match(
    jvmIdentifierClaimRecordExactnessBranch,
    /requiredExactString\(root\.get\("\{field\}"\), "identifier claim record\.\{field\}"\)[\s\S]*?requiredString\(root\.get\("\{field\}"\), "identifier claim record\.\{field\}"\)[\s\S]*?requiredExactString\(root\["\{field\}"\], "identifier claim record\.\{field\}"\)[\s\S]*?requiredString\(root\["\{field\}"\], "identifier claim record\.\{field\}"\)/u,
    "JVM identifier claim-record exactness negative control must mutate Java and Kotlin parser exact helpers",
  );
  assert.match(
    jvmIdentifierClaimRecordExactnessBranch,
    /identifierClaimRecordParserRejectsNonExactClaimFields[\s\S]*?identifierClaimRecordParserAllowsPaddedClaimFields[\s\S]*?identifier claim record parser must reject non-exact[\s\S]*?identifier claim record parser may normalize[\s\S]*?identifier claim record \$label exactness[\s\S]*?identifier claim record \$label may normalize/u,
    "JVM identifier claim-record exactness negative control must mutate Java and Kotlin padded-field regression markers",
  );
  assert.match(
    jvmIdentifierClaimRecordExactnessBranch,
    /Android Java identifier claim record policy-id exactness[\s\S]*?Kotlin identifier claim record policy-id exactness[\s\S]*?Android Java identifier policy metadata, proof-verifier, and claim record parser tests[\s\S]*?Kotlin identifier policy metadata, proof-verifier, and claim record parser tests[\s\S]*?JVM identifier claim-record exactness drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JVM identifier claim-record exactness drift was not detected"\)/u,
    "JVM identifier claim-record exactness negative control must only pass after detecting injected drift",
  );
  assert.match(
    guard,
    /normalizeIdentifierClaimLookupResponse[\s\S]*?identifier claim record exactness[\s\S]*?claimRecordFixture[\s\S]*?getIdentifierClaimByReceiptHash rejects non-exact claim record fields[\s\S]*?debugName: "identifier claim record\.policy_id"[\s\S]*?field: "identifier claim record\.receipt_hash"[\s\S]*?testGetIdentifierClaimByReceiptHashRejectsNonExactClaimFieldsAsync/u,
    "JS/Swift identifier claim-record guard must pin exact lookup helpers and padded-field tests",
  );
  const jsSwiftIdentifierClaimRecordExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-swift-identifier-claim-record-exactness":'),
    guard.indexOf('if mode == "--negative-control-ram-lfe-response-exactness":'),
  );
  assert.match(
    jsSwiftIdentifierClaimRecordExactnessBranch,
    /policy_id: requireIdentifierPolicyId\(record\.policy_id[\s\S]*?policy_id: requireNonEmptyString\(record\.policy_id[\s\S]*?opaque_id: requireExactReceiptPrefixedHash\(record\.opaque_id[\s\S]*?opaque_id: normalizeOpaqueLiteral\(record\.opaque_id[\s\S]*?account_id: requireExactAccountId\(record\.account_id[\s\S]*?account_id: ToriiClient\._requireAccountId\(record\.account_id/u,
    "JS/Swift identifier claim-record negative control must mutate JavaScript source and dist exact helpers",
  );
  assert.match(
    jsSwiftIdentifierClaimRecordExactnessBranch,
    /getIdentifierClaimByReceiptHash rejects non-exact claim record fields[\s\S]*?getIdentifierClaimByReceiptHash allows normalized claim record fields[\s\S]*?claim record \$\{field\} exactness[\s\S]*?claim record \$\{field\} may normalize/u,
    "JS/Swift identifier claim-record negative control must mutate JavaScript regression markers",
  );
  assert.match(
    jsSwiftIdentifierClaimRecordExactnessBranch,
    /ToriiIdentifierReceiptWireValue\.exactPolicyId\([\s\S]*?ToriiIdentifierReceiptWireValue\.normalizedPolicyId\([\s\S]*?ToriiIdentifierReceiptWireValue\.exactAccountId\([\s\S]*?ToriiIdentifierReceiptWireValue\.normalizedPolicyId\(/u,
    "JS/Swift identifier claim-record negative control must mutate Swift exact decode helpers",
  );
  assert.match(
    jsSwiftIdentifierClaimRecordExactnessBranch,
    /testGetIdentifierClaimByReceiptHashRejectsNonExactClaimFieldsAsync[\s\S]*?testGetIdentifierClaimByReceiptHashAllowsNormalizedClaimFieldsAsync[\s\S]*?expected non-exact claim record[\s\S]*?expected normalized claim record/u,
    "JS/Swift identifier claim-record negative control must mutate Swift regression markers",
  );
  assert.match(
    jsSwiftIdentifierClaimRecordExactnessBranch,
    /JavaScript identifier policy metadata, proof-verifier, and claim record exactness tests[\s\S]*?Swift identifier receipt account-id, attestation kind, and malformed proof base64 tests[\s\S]*?JS\/Swift identifier claim-record exactness drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JS\/Swift identifier claim-record exactness drift was not detected"\)/u,
    "JS/Swift identifier claim-record negative control must only pass after detecting injected drift",
  );
  assert.match(
    guard,
    /RAM-LFE execute response exactness[\s\S]*?RAM-LFE receipt verify response exactness[\s\S]*?JavaScript RAM-LFE response and program-policy exactness tests[\s\S]*?Android Java RAM-LFE execute response opaque-hash exactness[\s\S]*?Kotlin RAM-LFE execute response opaque-hash exactness[\s\S]*?testRamLfeResponseParsersRejectNonExactFieldsAsync/u,
    "RAM-LFE response exactness guard must pin source and regression markers across non-C# SDKs",
  );
  const ramLfeResponseExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-ram-lfe-response-exactness":'),
    guard.indexOf('if mode == "--negative-control-ram-lfe-program-policy-exactness":'),
  );
  assert.match(
    ramLfeResponseExactnessBranch,
    /requiredExactString\(root\.get\("program_id"\), "ram-lfe execute response\.program_id"\)[\s\S]*?requiredString\(root\.get\("program_id"\), "ram-lfe execute response\.program_id"\)[\s\S]*?canonicalizeExactHash32\(root\["opaque_hash"\], "ram-lfe execute response\.opaque_hash"\)[\s\S]*?requiredString\(root\["opaque_hash"\], "ram-lfe execute response\.opaque_hash"\)/u,
    "RAM-LFE response negative control must mutate Java and Kotlin response parsers",
  );
  assert.match(
    ramLfeResponseExactnessBranch,
    /program_id: requireExactNonEmptyString\(record\.program_id[\s\S]*?program_id: requireNonEmptyString\(record\.program_id[\s\S]*?output_ciphertext: requireExactHexString\([\s\S]*?output_ciphertext: requireHexString\(/u,
    "RAM-LFE response negative control must mutate JavaScript source and dist exact helpers",
  );
  assert.match(
    ramLfeResponseExactnessBranch,
    /testRamLfeResponseParsersRejectNonExactFieldsAsync[\s\S]*?testRamLfeResponseParsersAllowNormalizedFieldsAsync[\s\S]*?RAM-LFE response exactness drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: RAM-LFE response exactness drift was not detected"\)/u,
    "RAM-LFE response negative control must only pass after detecting injected drift",
  );
  assert.match(
    guard,
    /normalizeRamLfeProgramPolicySummary[\s\S]*?normalizeRamLfeProofVerifierMetadata[\s\S]*?RAM-LFE program policy exactness[\s\S]*?listRamLfeProgramPolicies rejects non-exact policy metadata[\s\S]*?listRamLfeProgramPolicies rejects non-exact proof-verifier metadata[\s\S]*?Swift RAM-LFE program policy exactness[\s\S]*?testRamLfeProgramPolicyParserRejectsNonExactFieldsAsync[\s\S]*?Android Java RAM-LFE program policy program-id exactness[\s\S]*?Kotlin RAM-LFE program policy program-id exactness/u,
    "RAM-LFE program-policy exactness guard must pin source and regression markers across non-C# SDKs",
  );
  const ramLfeProgramPolicyExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-ram-lfe-program-policy-exactness":'),
    guard.indexOf('if mode == "--negative-control-android-device-lab-family-fail-closed":'),
  );
  assert.match(
    ramLfeProgramPolicyExactnessBranch,
    /requiredExactString\(\\n\s+item\.get\("program_id"\),\\n\s+"ram-lfe program policy list\.items\[" \+ i \+ "\]\.program_id"\)[\s\S]*?requiredString\(\\n\s+item\.get\("program_id"\),\\n\s+"ram-lfe program policy list\.items\[" \+ i \+ "\]\.program_id"\)[\s\S]*?requiredExactString\(item\["program_id"\], "ram-lfe program policy list\.items\[\$i\]\.program_id"\)[\s\S]*?requiredString\(item\["program_id"\], "ram-lfe program policy list\.items\[\$i\]\.program_id"\)/u,
    "RAM-LFE program-policy negative control must mutate Java and Kotlin policy parsers",
  );
  assert.match(
    ramLfeProgramPolicyExactnessBranch,
    /const backend = requireExactNonEmptyString\(record\.backend[\s\S]*?const backend = requireNonEmptyString\(record\.backend[\s\S]*?owner: requireExactAccountId\(record\.owner[\s\S]*?owner: ToriiClient\._requireAccountId\(record\.owner[\s\S]*?result\.input_encryption_public_parameters = requireExactHexString\([\s\S]*?result\.input_encryption_public_parameters = requireHexString\([\s\S]*?result\.proof_verifier = normalizeRamLfeProofVerifierMetadata\([\s\S]*?result\.proof_verifier = record\.proof_verifier;/u,
    "RAM-LFE program-policy negative control must mutate JavaScript source/dist exact helpers and proof-verifier parsing",
  );
  assert.match(
    ramLfeProgramPolicyExactnessBranch,
    /listRamLfeProgramPolicies rejects non-exact proof-verifier metadata[\s\S]*?listRamLfeProgramPolicies allows normalized proof-verifier metadata[\s\S]*?RAM-LFE proof verifier \$\{field\} exactness[\s\S]*?RAM-LFE proof verifier \$\{field\} may normalize/u,
    "RAM-LFE program-policy negative control must mutate JavaScript proof-verifier regression markers",
  );
  assert.match(
    ramLfeProgramPolicyExactnessBranch,
    /testRamLfeProgramPolicyParserRejectsNonExactFieldsAsync[\s\S]*?testRamLfeProgramPolicyParserAllowsNormalizedFieldsAsync[\s\S]*?RAM-LFE program-policy exactness drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: RAM-LFE program-policy exactness drift was not detected"\)/u,
    "RAM-LFE program-policy negative control must only pass after detecting injected drift",
  );
  assert.match(
    guard,
    /normalizeIdentifierPolicySummary[\s\S]*?identifier policy proof-verifier exactness[\s\S]*?listIdentifierPolicies rejects non-exact proof-verifier metadata[\s\S]*?Swift identifier policy proof-verifier metadata tests[\s\S]*?Android Java identifier policy proof-verifier exactness[\s\S]*?Kotlin identifier policy proof-verifier exactness/u,
    "identifier policy proof-verifier exactness guard must pin source and regression markers across non-C# SDKs",
  );
  const identifierPolicyProofVerifierBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-identifier-policy-proof-verifier-exactness":'),
    guard.indexOf('if mode == "--negative-control-identifier-policy-metadata-exactness":'),
  );
  assert.match(
    identifierPolicyProofVerifierBranch,
    /requiredExactString\(root\.get\("proof_backend"\), context \+ "\.proof_backend"\)[\s\S]*?requiredString\(root\.get\("proof_backend"\), context \+ "\.proof_backend"\)[\s\S]*?requiredExactString\(root\["proof_backend"\], "\$context\.proof_backend"\)[\s\S]*?requiredString\(root\["proof_backend"\], "\$context\.proof_backend"\)/u,
    "identifier policy proof-verifier negative control must mutate Java and Kotlin parser exactness",
  );
  assert.match(
    identifierPolicyProofVerifierBranch,
    /result\.proof_verifier = normalizeRamLfeProofVerifierMetadata\([\s\S]*?result\.proof_verifier = record\.proof_verifier;/u,
    "identifier policy proof-verifier negative control must mutate JavaScript preservation",
  );
  assert.match(
    identifierPolicyProofVerifierBranch,
    /listIdentifierPolicies rejects non-exact proof-verifier metadata[\s\S]*?listIdentifierPolicies allows normalized proof-verifier metadata[\s\S]*?identifier policy proof verifier \$\{field\} exactness[\s\S]*?identifier policy proof verifier \$\{field\} may normalize[\s\S]*?identifier policy proof-verifier exactness drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: identifier policy proof-verifier exactness drift was not detected"\)/u,
    "identifier policy proof-verifier negative control must mutate regression markers and only pass after detecting drift",
  );
  assert.match(
    guard,
    /normalizeIdentifierBfvPublicParameters[\s\S]*?identifier BFV metadata exactness[\s\S]*?normalizeIdentifierPolicySummary[\s\S]*?identifier policy metadata exactness[\s\S]*?listIdentifierPolicies rejects non-exact policy metadata[\s\S]*?Swift identifier policy metadata exactness[\s\S]*?Android Java identifier policy metadata exactness[\s\S]*?Kotlin identifier policy metadata exactness/u,
    "identifier policy metadata exactness guard must pin source and regression markers across non-C# SDKs",
  );
  const identifierPolicyMetadataBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-identifier-policy-metadata-exactness":'),
    guard.indexOf('if mode == "--negative-control-android-device-lab-family-fail-closed":'),
  );
  assert.match(
    identifierPolicyMetadataBranch,
    /requiredExactString\(item\.get\("owner"\), "identifier policy list\.items\[" \+ i \+ "\]\.owner"\)[\s\S]*?requiredString\(item\.get\("owner"\), "identifier policy list\.items\[" \+ i \+ "\]\.owner"\)[\s\S]*?requiredExactString\(item\["owner"\], "identifier policy list\.items\[\$i\]\.owner"\)[\s\S]*?requiredString\(item\["owner"\], "identifier policy list\.items\[\$i\]\.owner"\)/u,
    "identifier policy metadata negative control must mutate Java and Kotlin owner exactness",
  );
  assert.match(
    identifierPolicyMetadataBranch,
    /norito_length_encoding = requireExactNonEmptyString\([\s\S]*?norito_length_encoding = requireNonEmptyString\([\s\S]*?const normalization = requireExactNonEmptyString\([\s\S]*?const normalization = requireNonEmptyString\([\s\S]*?owner: requireExactAccountId\(record\.owner[\s\S]*?owner: ToriiClient\._requireAccountId\(record\.owner[\s\S]*?result\.input_encryption_public_parameters = requireExactHexString\([\s\S]*?result\.input_encryption_public_parameters = requireHexString\(/u,
    "identifier policy metadata negative control must mutate JavaScript exact helpers",
  );
  assert.match(
    identifierPolicyMetadataBranch,
    /testListIdentifierPoliciesRejectsNonExactMetadata[\s\S]*?testListIdentifierPoliciesAllowsNormalizedMetadata/u,
    "identifier policy metadata negative control must mutate Swift regression markers",
  );
  assert.match(
    identifierPolicyMetadataBranch,
    /listIdentifierPolicies rejects non-exact policy metadata[\s\S]*?listIdentifierPolicies allows normalized policy metadata/u,
    "identifier policy metadata negative control must mutate JavaScript policy regression title",
  );
  assert.match(
    identifierPolicyMetadataBranch,
    /identifier policy metadata \$\{field\} exactness[\s\S]*?identifier policy metadata \$\{field\} may normalize/u,
    "identifier policy metadata negative control must mutate JavaScript parameterized regression marker",
  );
  assert.match(
    identifierPolicyMetadataBranch,
    /identifier policy metadata exactness drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: identifier policy metadata exactness drift was not detected"\)/u,
    "identifier policy metadata negative control must only pass after detecting drift",
  );
  assert.match(
    guard,
    /normalizeAliasResolutionResponse[\s\S]*?account alias resolution exactness[\s\S]*?JavaScript account alias resolution exactness tests[\s\S]*?Swift account alias resolution exactness[\s\S]*?Android Java account alias resolution alias exactness[\s\S]*?Kotlin account alias resolution alias exactness/u,
    "account alias resolution exactness guard must pin source and regression markers across non-C# SDKs",
  );
  const accountAliasResolutionBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-account-alias-resolution-exactness":'),
    guard.indexOf('if mode == "--negative-control-android-device-lab-family-fail-closed":'),
  );
  assert.match(
    accountAliasResolutionBranch,
    /const alias = requireExactNonEmptyString\(record\.alias[\s\S]*?const alias = requireNonEmptyString\(record\.alias[\s\S]*?const rawAccountId = requireExactNonEmptyString\([\s\S]*?const rawAccountId = requireNonEmptyString\([\s\S]*?result\.source = requireExactNonEmptyString\(sourceValue[\s\S]*?result\.source = requireNonEmptyString\(sourceValue/u,
    "account alias resolution negative control must mutate JavaScript exact helpers",
  );
  assert.match(
    accountAliasResolutionBranch,
    /requiredExactString\(root\.get\("alias"\), "account alias resolution\.alias"\)[\s\S]*?requiredString\(root\.get\("alias"\), "account alias resolution\.alias"\)[\s\S]*?requiredExactString\(root\["alias"\], "account alias resolution\.alias"\)[\s\S]*?requiredString\(root\["alias"\], "account alias resolution\.alias"\)/u,
    "account alias resolution negative control must mutate Java and Kotlin alias exactness",
  );
  assert.match(
    accountAliasResolutionBranch,
    /resolveAlias parses exact payload fields[\s\S]*?resolveAlias normalizes payload fields[\s\S]*?accountAliasParserRejectsNonExactResponseFields[\s\S]*?accountAliasParserAllowsNormalizedResponseFields[\s\S]*?testResolveAccountAliasRejectsNonExactResponseFields[\s\S]*?testResolveAccountAliasAllowsNormalizedResponseFields/u,
    "account alias resolution negative control must mutate regression markers",
  );
  assert.match(
    accountAliasResolutionBranch,
    /account alias resolution exactness drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: account alias resolution exactness drift was not detected"\)/u,
    "account alias resolution negative control must only pass after detecting drift",
  );
  assert.match(
    guard,
    /normalizeMultisigContractCallResponse[\s\S]*?multisig resolved account exactness[\s\S]*?JavaScript multisig resolved account exactness tests[\s\S]*?Swift multisig resolved account exactness[\s\S]*?Python multisig resolved account exactness[\s\S]*?Android Java multisig resolved account exactness[\s\S]*?Kotlin multisig resolved account exactness/u,
    "multisig resolved account exactness guard must pin source and regression markers across non-C# SDKs",
  );
  const multisigResolvedAccountBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-multisig-resolved-account-exactness":'),
    guard.indexOf('if mode == "--negative-control-android-device-lab-family-fail-closed":'),
  );
  assert.match(
    multisigResolvedAccountBranch,
    /resolved_multisig_account_id: requireExactAccountId\([\s\S]*?resolved_multisig_account_id: ToriiClient\._normalizeAccountId\(/u,
    "multisig resolved account negative control must mutate JavaScript exact helpers",
  );
  assert.match(
    multisigResolvedAccountBranch,
    /requiredExactAccountId\(root\.get\("resolved_multisig_account_id"\), "multisig response\.resolved_multisig_account_id"\)[\s\S]*?requiredString\(root\.get\("resolved_multisig_account_id"\), "multisig response\.resolved_multisig_account_id"\)[\s\S]*?requiredExactAccountId\(root\["resolved_multisig_account_id"\], "multisig response\.resolved_multisig_account_id"\)[\s\S]*?requiredString\(root\["resolved_multisig_account_id"\], "multisig response\.resolved_multisig_account_id"\)/u,
    "multisig resolved account negative control must mutate Java and Kotlin exact helpers",
  );
  assert.match(
    multisigResolvedAccountBranch,
    /decodeExactToriiAccountId[\s\S]*?decodeNormalizedToriiAccountId[\s\S]*?guard !raw\.contains\("@"\)[\s\S]*?guard raw\.contains\("@"\) \|\| true/u,
    "multisig resolved account negative control must mutate Swift exact helper markers",
  );
  assert.match(
    multisigResolvedAccountBranch,
    /_require_exact_i105_account_id[\s\S]*?_normalize_canonical_account_id[\s\S]*?test_propose_multisig_rejects_malformed_response_fields[\s\S]*?test_propose_multisig_allows_normalized_response_fields/u,
    "multisig resolved account negative control must mutate Python exact helper markers",
  );
  assert.match(
    multisigResolvedAccountBranch,
    /multisig response decoders reject non-exact resolved account ids[\s\S]*?multisig response decoders allow normalized resolved account ids[\s\S]*?testMultisigResponsesRejectNonExactResolvedAccountIds[\s\S]*?testMultisigResponsesAllowNormalizedResolvedAccountIds[\s\S]*?padded resolved multisig account id must be rejected[\s\S]*?padded resolved multisig account id may normalize[\s\S]*?testMultisigAccountId[\s\S]*?testNormalizedMultisigAccountId/u,
    "multisig resolved account negative control must mutate regression markers",
  );
  assert.match(
    multisigResolvedAccountBranch,
    /multisig resolved account exactness drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: multisig resolved account exactness drift was not detected"\)/u,
    "multisig resolved account negative control must only pass after detecting drift",
  );
  assert.match(
    guard,
    /KagemushaDeviceLabArtifactExportTest\.java[\s\S]*?attached device is not in the standard Kagemusha production matrix[\s\S]*?Android device-lab exporter must fail closed after the standard device matrix/u,
    "SDK parity guard must pin Android device-lab exporter fail-closed family inference",
  );
  const androidDeviceLabFamilyBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-android-device-lab-family-fail-closed":'),
    guard.indexOf('if mode == "--negative-control-android-device-lab-family-overmatch":'),
  );
  assert.match(
    androidDeviceLabFamilyBranch,
    /attached device is not in the standard Kagemusha production matrix[\s\S]*?return "Google Pixel 6 \/ 6a";/u,
    "Android device-lab family negative control must reintroduce the Pixel fallback",
  );
  assert.match(
    androidDeviceLabFamilyBranch,
    /Android device-lab exporter must fail closed[\s\S]*?Android device-lab family fallback drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Android device-lab family fallback drift was not detected"\)/u,
    "Android device-lab family negative control must only pass after detecting fallback drift",
  );
  const androidDeviceLabFamilyOvermatchBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-android-device-lab-family-overmatch":'),
    guard.indexOf('if mode == "--negative-control-android-device-lab-family-override-binding":'),
  );
  assert.match(
    guard,
    /scripts\/kagemusha_android_device_lab_slot\.py[\s\S]*?device family must match attached device model\/codename[\s\S]*?model_family is None or codename_family is None[\s\S]*?model_family != codename_family[\s\S]*?test_kagemusha_slot_assembler_rejects_family_override_mismatch[\s\S]*?test_kagemusha_slot_assembler_rejects_conflicting_model_codename[\s\S]*?test_production_metadata_rejects_unknown_model_with_known_codename[\s\S]*?Android slot assembler exact family inference tests/u,
    "SDK parity guard must pin exact host-side Android family inference, conflict rejection, and override binding tests",
  );
  assert.match(
    androidDeviceLabFamilyOvermatchBranch,
    /&& isExactDevice\(device, exactDevices\)[\s\S]*?\|\| isExactDevice\(device, exactDevices\)/u,
    "Android device-lab overmatch negative control must break paired model/device matching",
  );
  assert.match(
    androidDeviceLabFamilyOvermatchBranch,
    /Android device-lab exporter exact family matching[\s\S]*?Android device-lab family overmatch drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Android device-lab family overmatch drift was not detected"\)/u,
    "Android device-lab overmatch negative control must only pass after detecting substring drift",
  );
  const androidDeviceLabFamilyOverrideBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-android-device-lab-family-override-binding":'),
    guard.indexOf('if mode == "--negative-control-android-device-lab-assembler-identity-fields":'),
  );
  assert.match(
    androidDeviceLabFamilyOverrideBranch,
    /if has_device_identity and inferred != family:[\s\S]*?device family must match attached device model\/codename[\s\S]*?""/u,
    "Android device-lab override negative control must remove the requested-family identity binding",
  );
  assert.match(
    androidDeviceLabFamilyOverrideBranch,
    /Android slot assembler exact family matching[\s\S]*?Android device-lab family override drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Android device-lab family override drift was not detected"\)/u,
    "Android device-lab override negative control must only pass after detecting binding drift",
  );
  const androidDeviceLabAssemblerIdentityBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-android-device-lab-assembler-identity-fields":'),
    guard.indexOf('if mode == "--negative-control-native-bridge-zero-envelope-pallas-guard":'),
  );
  assert.match(
    guard,
    /scripts\/kagemusha_android_device_lab_slot\.py[\s\S]*?"device_model": facts\["device_model"\][\s\S]*?test_production_metadata_rejects_slot_family_model_codename_mismatch[\s\S]*?test_production_metadata_rejects_conflicting_model_codename[\s\S]*?Android slot assembler exact family inference tests/u,
    "SDK parity guard must pin Android assembler signed model/codename fields and identity regression tests",
  );
  assert.match(
    androidDeviceLabAssemblerIdentityBranch,
    /"device_model": facts\["device_model"\][\s\S]*?"device_model": family/u,
    "Android device-lab assembler identity negative control must replace signed model fields",
  );
  assert.match(
    androidDeviceLabAssemblerIdentityBranch,
    /Android slot assembler exact family matching[\s\S]*?Android device-lab assembler identity field drift was not detected[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Android device-lab assembler identity field drift was not detected"\)/u,
    "Android device-lab assembler identity negative control must only pass after detecting field drift",
  );

  const expectedPrivacyWorkflowPaths = [
    "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt",
    "javascript/iroha_js/src/crypto.js",
    "javascript/iroha_js/dist/crypto.js",
    "javascript/iroha_js/test/privacyCatalogParity.test.js",
    "javascript/iroha_js/test/privacyNative.test.js",
    "python/iroha_python/src/iroha_python/crypto.py",
    "python/iroha_python/src/iroha_python/privacy_catalog.py",
    "python/iroha_python/tests/privacy_catalog_test.py",
    "python/iroha_python/tests/crypto_algorithms_test.py",
    "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs",
  ];
  assert.deepStrictEqual(
    quotedStringsFromInventory(
      guard,
      "SDK_PRIVACY_WORKFLOW_INVENTORY_PATHS = (",
      "SDK_PARITY_MAIN_COMMAND =",
    ),
    [...expectedPrivacyWorkflowPaths].sort(),
    "SDK privacy workflow inventory matrix must pin every SDK privacy/native trigger path",
  );
  assertWorkflowIncludesPaths(
    workflow,
    expectedPrivacyWorkflowPaths,
    "Kagemusha SDK privacy workflow inventory matrix",
  );
  const privacyWorkflowInventoryMatrixBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-privacy-workflow-inventory-matrix":'),
    guard.indexOf('if mode == "--negative-control-js-sdk-install-order-workflow":'),
  );
  assert.match(
    privacyWorkflowInventoryMatrixBranch,
    /for relative in SDK_PRIVACY_WORKFLOW_INVENTORY_PATHS:[\s\S]*if relative not in message:[\s\S]*rejected\.append\(relative\)/u,
    "SDK privacy workflow inventory matrix must require each missing path to be named by the guard",
  );
  assert.match(
    privacyWorkflowInventoryMatrixBranch,
    /else:[\s\S]*negative control failed: SDK privacy workflow inventory drift was not detected for /u,
    "SDK privacy workflow inventory matrix must fail if any path removal is not detected",
  );
  assert.match(
    privacyWorkflowInventoryMatrixBranch,
    /text_overrides\.pop\(target, None\)[\s\S]*checked \{len\(rejected\)\} SDK privacy workflow paths/u,
    "SDK privacy workflow inventory matrix must clear overrides and report checked path count",
  );

  const preferredModeFallbackBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-cross-sdk-preferred-mode-fallback":'),
    guard.indexOf('if mode == "--negative-control-rust-recursive-compact-unavailable-classifier":'),
  );
  assert.match(
    preferredModeFallbackBranch,
    /recursiveCompactAvailable[\s\S]*KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1[\s\S]*recursive_compact_available[\s\S]*RECURSIVE_COMPACT_V1/u,
    "preferred-mode fallback negative control must inject compact-default drift across SDKs",
  );
  assert.match(
    preferredModeFallbackBranch,
    /Swift preferred Kagemusha mode fallback policy[\s\S]*Android Java preferred Kagemusha mode fallback policy[\s\S]*C# preferred Kagemusha mode fallback policy/u,
    "preferred-mode fallback negative control must expect mobile and C# fallback-policy labels",
  );
  assert.match(
    preferredModeFallbackBranch,
    /run_checks\(mutated\)/u,
    "preferred-mode fallback negative control must validate the mutated text snapshot",
  );
  assert.match(
    preferredModeFallbackBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*raise\s+SystemExit\(0\)[\s\S]*raise\s+SystemExit\("negative control failed: cross-SDK preferred-mode fallback drift was not detected"\)/u,
    "preferred-mode fallback negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    preferredModeFallbackBranch,
    /\n    raise SystemExit\(0\)\n    raise SystemExit\("negative control failed: cross-SDK preferred-mode fallback drift was not detected"\)/u,
    "preferred-mode fallback negative control must not pass after an undetected run_checks result",
  );

  const nativeBridgeTestWorkflowBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-native-bridge-test-workflow":'),
    guard.indexOf('if mode == "--negative-control-native-bridge-windowed-record-order-workflow":'),
  );
  assert.match(
    guard,
    /NATIVE_BRIDGE_RECURSIVE_COMPACT_TEST_COMMAND = "RUST_MIN_STACK=67108864 CARGO_PROFILE_TEST_OPT_LEVEL=3 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p connect_norito_bridge kagemusha_recursive_compact_ffi_fails_closed_and_rejects_adversarial_inputs --lib -- --test-threads=1"/u,
    "heavyweight native recursive compact bridge test must use the optimized Cargo test profile",
  );
  assert.match(
    nativeBridgeTestWorkflowBranch,
    /NATIVE_BRIDGE_EMPTY_NESTED_PALLAS_TEST_COMMAND[\s\S]*?--skip kagemusha_recursive_spend_ffi_rejects_empty_nested_pallas[\s\S]*?native recursive spend empty nested-Pallas bridge test/u,
    "native bridge workflow negative control must mutate the empty nested-Pallas bridge test command",
  );
  assert.match(
    nativeBridgeTestWorkflowBranch,
    /NATIVE_BRIDGE_RECURSIVE_COMPACT_WINDOWED_RECORD_TEST_COMMAND[\s\S]*?--skip kagemusha_recursive_compact_ffi_rejects_windowed_records[\s\S]*?native recursive compact windowed-record bridge test/u,
    "native bridge workflow negative control must mutate the recursive compact windowed-record bridge test command",
  );
  assert.match(
    nativeBridgeTestWorkflowBranch,
    /JS_HOST_APPEND_BOUNDARY_TEST_COMMAND[\s\S]*?--skip kagemusha_recursive_spend_lineage_append_boundary[\s\S]*?JS host append-boundary duplicate-output test/u,
    "native bridge workflow negative control must mutate the JS host append-boundary duplicate-output test command",
  );
  const nativeBridgeWindowedRecordOrderWorkflowBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-native-bridge-windowed-record-order-workflow":'),
    guard.indexOf('if mode == "--negative-control-native-bridge-needs-workflow":'),
  );
  assert.match(
    nativeBridgeWindowedRecordOrderWorkflowBranch,
    /windowed_line \+ heavyweight_line[\s\S]*?heavyweight_line \+ windowed_line[\s\S]*?windowed-record bridge test before the heavyweight recursive compact adversarial test/u,
    "native bridge workflow ordering negative control must swap the windowed-record and heavyweight recursive compact commands",
  );

  const browserHelperBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-browser-helper":'),
    guard.indexOf('if mode == "--negative-control-js-lineage-key-artifact-copy":'),
  );
  assert.match(
    browserHelperBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: browser helper drift was not detected"\)/u,
    "browser-helper negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    browserHelperBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "browser-helper negative control must not unconditionally pass after run_checks",
  );
  const jsLineageKeyArtifactCopyBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-lineage-key-artifact-copy":'),
    guard.indexOf('if mode == "--negative-control-js-lineage-key-package-binding":'),
  );
  assert.match(
    jsLineageKeyArtifactCopyBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JS lineage key artifact copy drift was not detected"\)/u,
    "JS lineage key artifact copy negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsLineageKeyArtifactCopyBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS lineage key artifact copy negative control must not unconditionally pass after run_checks",
  );
  const jsLineageKeyPackageBindingBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-lineage-key-package-binding":'),
    guard.indexOf('if mode == "--negative-control-python-lineage-key-package-binding":'),
  );
  assert.match(
    jsLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JS lineage key package binding drift was not detected"\)/u,
    "JS lineage key package binding negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS lineage key package binding negative control must not unconditionally pass after run_checks",
  );
  const pythonLineageKeyPackageBindingBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-python-lineage-key-package-binding":'),
    guard.indexOf('if mode == "--negative-control-csharp-lineage-key-package-binding":'),
  );
  assert.match(
    pythonLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Python lineage key package binding drift was not detected"\)/u,
    "Python lineage key package binding negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    pythonLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Python lineage key package binding negative control must not unconditionally pass after run_checks",
  );
  const csharpLineageKeyPackageBindingBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-csharp-lineage-key-package-binding":'),
    guard.indexOf('if mode == "--negative-control-csharp-lineage-cid1-exactness":'),
  );
  assert.match(
    csharpLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: C# lineage key package binding drift was not detected"\)/u,
    "C# lineage key package binding negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    csharpLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "C# lineage key package binding negative control must not unconditionally pass after run_checks",
  );
  const csharpLineageCid1ExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-csharp-lineage-cid1-exactness":'),
    guard.indexOf('if mode == "--negative-control-csharp-canonical-request-exactness":'),
  );
  assert.match(
    csharpLineageCid1ExactnessBranch,
    /Encoding\.UTF8\.GetString\(payload\);[\s\S]*?Encoding\.UTF8\.GetString\(payload\)\.Trim\(\);/u,
    "C# lineage CID1 exactness negative control must reintroduce whitespace normalization",
  );
  assert.match(
    csharpLineageCid1ExactnessBranch,
    /whitespaceCidVerifierKey[\s\S]*?normalizedCidVerifierKey/u,
    "C# lineage CID1 exactness negative control must mutate padded CID1 coverage",
  );
  assert.match(
    csharpLineageCid1ExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: C# lineage CID1 exactness drift was not detected"\)/u,
    "C# lineage CID1 exactness negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    csharpLineageCid1ExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "C# lineage CID1 exactness negative control must not unconditionally pass after run_checks",
  );
  const csharpCanonicalRequestExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-csharp-canonical-request-exactness":'),
    guard.indexOf('if mode == "--negative-control-csharp-identifier-receipt-exactness":'),
  );
  assert.match(
    csharpCanonicalRequestExactnessBranch,
    /RequireExactNonBlank\(nonce, nameof\(nonce\)\)[\s\S]*?string\.IsNullOrWhiteSpace\(nonce\) \? GenerateNonce\(\) : nonce/u,
    "C# canonical request exactness negative control must reintroduce blank nonce generation",
  );
  assert.match(
    csharpCanonicalRequestExactnessBranch,
    /CanonicalRequestAuthRejectsPaddedAndBlankFields[\s\S]*?CanonicalRequestAuthAllowsPaddedAndBlankFields/u,
    "C# canonical request exactness negative control must mutate padded/blank auth coverage",
  );
  assert.match(
    csharpCanonicalRequestExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: C# canonical request exactness drift was not detected"\)/u,
    "C# canonical request exactness negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    csharpCanonicalRequestExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "C# canonical request exactness negative control must not unconditionally pass after run_checks",
  );
  const csharpIdentifierReceiptExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-csharp-identifier-receipt-exactness":'),
    guard.indexOf('if mode == "--negative-control-csharp-transaction-builder-exactness":'),
  );
  assert.match(
    csharpIdentifierReceiptExactnessBranch,
    /ToriiIdentifierResolveResponseJsonConverter[\s\S]*?ToriiIdentifierResolveResponseUncheckedJsonConverter/u,
    "C# identifier receipt exactness negative control must remove the checked response converter marker",
  );
  assert.match(
    csharpIdentifierReceiptExactnessBranch,
    /IdentifierResolveResponseRejectsPaddedReceiptFields[\s\S]*?IdentifierResolveResponseAllowsPaddedReceiptFields/u,
    "C# identifier receipt exactness negative control must mutate padded receipt coverage",
  );
  assert.match(
    csharpIdentifierReceiptExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: C# identifier receipt exactness drift was not detected"\)/u,
    "C# identifier receipt exactness negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    csharpIdentifierReceiptExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "C# identifier receipt exactness negative control must not unconditionally pass after run_checks",
  );
  const csharpTransactionBuilderExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-csharp-transaction-builder-exactness":'),
    guard.indexOf('if mode == "--negative-control-csharp-transaction-encoding-exactness":'),
  );
  assert.match(
    csharpTransactionBuilderExactnessBranch,
    /return value;[\s\S]*?return value\.Trim\(\);/u,
    "C# transaction builder exactness negative control must reintroduce Trim normalization",
  );
  assert.match(
    csharpTransactionBuilderExactnessBranch,
    /TransactionBuilderRejectsPaddedTopLevelFields[\s\S]*?TransactionBuilderAllowsPaddedTopLevelFields/u,
    "C# transaction builder exactness negative control must mutate padded top-level coverage",
  );
  assert.match(
    csharpTransactionBuilderExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: C# transaction builder exactness drift was not detected"\)/u,
    "C# transaction builder exactness negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    csharpTransactionBuilderExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "C# transaction builder exactness negative control must not unconditionally pass after run_checks",
  );
  const csharpTransactionEncodingExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-csharp-transaction-encoding-exactness":'),
    guard.indexOf('if mode == "--negative-control-csharp-pallas-builder-surface":'),
  );
  assert.match(
    csharpTransactionEncodingExactnessBranch,
    /RequireExactNonBlank\(chainId, nameof\(chainId\)\)[\s\S]*?chainId\.Trim\(\)/u,
    "C# transaction encoding exactness negative control must reintroduce chain-id Trim normalization",
  );
  assert.match(
    csharpTransactionEncodingExactnessBranch,
    /TransactionEncodingContextRejectsPaddedBoundaryFields[\s\S]*?TransactionEncodingContextAllowsPaddedBoundaryFields/u,
    "C# transaction encoding exactness negative control must mutate padded boundary coverage",
  );
  assert.match(
    csharpTransactionEncodingExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: C# transaction encoding exactness drift was not detected"\)/u,
    "C# transaction encoding exactness negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    csharpTransactionEncodingExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "C# transaction encoding exactness negative control must not unconditionally pass after run_checks",
  );
  const csharpPallasBuilderBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-csharp-pallas-builder-surface":'),
    guard.indexOf('if mode == "--negative-control-swift-lineage-key-package-binding":'),
  );
  assert.match(
    csharpPallasBuilderBranch,
    /BuildPallasOpenEnvelopesArchive[\s\S]*?BuildPallasOpenMetadataArchive[\s\S]*?BuildPreviousProofOpenEnvelopesArchive[\s\S]*?BuildPreviousProofOpenMetadataArchive[\s\S]*?IsPallasOpenEnvelopeBuilderAvailable[\s\S]*?IsPallasOpenMetadataBuilderAvailable/u,
    "C# Pallas builder negative control must mutate the builder source surface",
  );
  assert.match(
    csharpPallasBuilderBranch,
    /PallasOpenEnvelopeBuildersRejectMalformedInputsBeforeLoadingNativeBridge[\s\S]*?PallasOpenMetadataBuildersRejectMalformedInputsBeforeLoadingNativeBridge/u,
    "C# Pallas builder negative control must mutate focused builder test coverage",
  );
  assert.match(
    csharpPallasBuilderBranch,
    /PallasOpenEnvelopeBuilderReadBridgeOutputRejectsMalformedNoritoSuccessOutput[\s\S]*?PallasOpenMetadataBuilderReadBridgeOutputRejectsMalformedNoritoSuccessOutput[\s\S]*?connect_norito_kagemusha_build_pallas_open_envelopes_archive returned invalid Norito archive[\s\S]*?connect_norito_kagemusha_build_pallas_open_envelopes_archive returned unchecked bytes/u,
    "C# Pallas builder negative control must mutate focused builder native-output coverage",
  );
  assert.match(
    csharpPallasBuilderBranch,
    /C# recursive compact wrapper missing[\s\S]*?C# recursive compact verifier tests missing[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: C# Pallas builder surface drift was not detected"\)/u,
    "C# Pallas builder negative control must only pass after source and test drift are detected",
  );
  assert.doesNotMatch(
    csharpPallasBuilderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "C# Pallas builder negative control must not unconditionally pass after run_checks",
  );
  const swiftLineageKeyPackageBindingBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-lineage-key-package-binding":'),
    guard.indexOf('if mode == "--negative-control-csharp-lineage-witness-availability-probe":'),
  );
  assert.match(
    swiftLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift lineage key package binding drift was not detected"\)/u,
    "Swift lineage key package binding negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift lineage key package binding negative control must not unconditionally pass after run_checks",
  );
  const csharpLineageWitnessAvailabilityProbeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-csharp-lineage-witness-availability-probe":'),
    guard.indexOf('if mode == "--negative-control-csharp-lineage-witness-append-availability-probe":'),
  );
  assert.match(
    csharpLineageWitnessAvailabilityProbeBranch,
    /Probe\(\(NativeArchivePairCall\)NativeLineageWitnessFromInitResult\)[\s\S]*?Probe\(\(NativeArchivePairCall\)NativeAppend\)[\s\S]*?run_checks\(mutated\)/u,
    "C# lineage witness availability negative control must mutate the actual init-result probe call",
  );
  assert.match(
    csharpLineageWitnessAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: C# lineage witness availability probe drift was not detected"\)/u,
    "C# lineage witness availability negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    csharpLineageWitnessAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "C# lineage witness availability negative control must not unconditionally pass after run_checks",
  );
  const csharpLineageWitnessAppendAvailabilityProbeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-csharp-lineage-witness-append-availability-probe":'),
    guard.indexOf('if mode == "--negative-control-swift-lineage-witness-availability-probe":'),
  );
  assert.match(
    csharpLineageWitnessAppendAvailabilityProbeBranch,
    /Probe\(\(NativeArchiveTripleCall\)NativeLineageWitnessAppendResult\)[\s\S]*?Probe\(\(NativeArchiveTripleCall\)NativeAppend\)[\s\S]*?run_checks\(mutated\)/u,
    "C# lineage witness append availability negative control must mutate the actual append probe call",
  );
  assert.match(
    csharpLineageWitnessAppendAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: C# lineage witness append availability probe drift was not detected"\)/u,
    "C# lineage witness append availability negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    csharpLineageWitnessAppendAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "C# lineage witness append availability negative control must not unconditionally pass after run_checks",
  );
  const swiftLineageWitnessAvailabilityProbeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-lineage-witness-availability-probe":'),
    guard.indexOf('if mode == "--negative-control-swift-lineage-witness-append-availability-probe":'),
  );
  assert.match(
    swiftLineageWitnessAvailabilityProbeBranch,
    /probeKagemushaLineageWitnessFromInitResultFunction\(\\n                kagemushaRecursiveSpendLineageWitnessFromInitResultFn[\s\S]*?probeKagemushaLineageWitnessFromInitResultFunction\(\\n                kagemushaRecursiveSpendInitFn[\s\S]*?run_checks\(mutated\)/u,
    "Swift lineage witness availability negative control must mutate the actual init-result probe call",
  );
  assert.match(
    swiftLineageWitnessAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift lineage witness availability probe drift was not detected"\)/u,
    "Swift lineage witness availability negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftLineageWitnessAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift lineage witness availability negative control must not unconditionally pass after run_checks",
  );
  const swiftLineageWitnessAppendAvailabilityProbeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-lineage-witness-append-availability-probe":'),
    guard.indexOf('if mode == "--negative-control-jvm-lineage-key-package-binding":'),
  );
  assert.match(
    swiftLineageWitnessAppendAvailabilityProbeBranch,
    /probeKagemushaLineageWitnessAppendResultFunction\(\\n                kagemushaRecursiveSpendLineageWitnessAppendResultFn[\s\S]*?probeKagemushaLineageWitnessAppendResultFunction\(\\n                kagemushaRecursiveSpendAppendFn[\s\S]*?run_checks\(mutated\)/u,
    "Swift lineage witness append availability negative control must mutate the actual append probe call",
  );
  assert.match(
    swiftLineageWitnessAppendAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift lineage witness append availability probe drift was not detected"\)/u,
    "Swift lineage witness append availability negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftLineageWitnessAppendAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift lineage witness append availability negative control must not unconditionally pass after run_checks",
  );
  const jvmLineageKeyPackageBindingBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-jvm-lineage-key-package-binding":'),
    guard.indexOf('if mode == "--negative-control-android-lineage-key-package-binding":'),
  );
  assert.match(
    jvmLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Kotlin\/JVM lineage key package binding drift was not detected"\)/u,
    "Kotlin/JVM lineage key package binding negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jvmLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Kotlin/JVM lineage key package binding negative control must not unconditionally pass after run_checks",
  );
  const androidLineageKeyPackageBindingBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-android-lineage-key-package-binding":'),
    guard.indexOf('if mode == "--negative-control-jvm-lineage-witness-availability-probe":'),
  );
  assert.match(
    androidLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Android Java lineage key package binding drift was not detected"\)/u,
    "Android Java lineage key package binding negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    androidLineageKeyPackageBindingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Android Java lineage key package binding negative control must not unconditionally pass after run_checks",
  );
  const jvmLineageWitnessAvailabilityProbeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-jvm-lineage-witness-availability-probe":'),
    guard.indexOf('if mode == "--negative-control-jvm-lineage-witness-append-availability-probe":'),
  );
  assert.match(
    jvmLineageWitnessAvailabilityProbeBranch,
    /nativeLineageWitnessFromInitResult\(probe, probe\)[\s\S]*?nativeLineageWitnessFromInitResult\(ByteArray\(0\), ByteArray\(0\)\)[\s\S]*?run_checks\(mutated\)/u,
    "Kotlin/JVM lineage witness availability negative control must mutate the actual probe call",
  );
  assert.match(
    jvmLineageWitnessAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Kotlin\/JVM lineage witness availability probe drift was not detected"\)/u,
    "Kotlin/JVM lineage witness availability negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jvmLineageWitnessAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Kotlin/JVM lineage witness availability negative control must not unconditionally pass after run_checks",
  );
  const jvmLineageWitnessAppendAvailabilityProbeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-jvm-lineage-witness-append-availability-probe":'),
    guard.indexOf('if mode == "--negative-control-android-lineage-witness-availability-probe":'),
  );
  assert.match(
    jvmLineageWitnessAppendAvailabilityProbeBranch,
    /nativeLineageWitnessAppendResult\(probe, probe, probe\)[\s\S]*?nativeLineageWitnessAppendResult\(ByteArray\(0\), ByteArray\(0\), ByteArray\(0\)\)[\s\S]*?run_checks\(mutated\)/u,
    "Kotlin/JVM lineage witness append availability negative control must mutate the actual append probe call",
  );
  assert.match(
    jvmLineageWitnessAppendAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Kotlin\/JVM lineage witness append availability probe drift was not detected"\)/u,
    "Kotlin/JVM lineage witness append availability negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jvmLineageWitnessAppendAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Kotlin/JVM lineage witness append availability negative control must not unconditionally pass after run_checks",
  );
  const androidLineageWitnessAvailabilityProbeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-android-lineage-witness-availability-probe":'),
    guard.indexOf('if mode == "--negative-control-android-lineage-witness-append-availability-probe":'),
  );
  assert.match(
    androidLineageWitnessAvailabilityProbeBranch,
    /nativeLineageWitnessFromInitResult\(probe, probe\)[\s\S]*?nativeLineageWitnessFromInitResult\(new byte\[0\], new byte\[0\]\)[\s\S]*?run_checks\(mutated\)/u,
    "Android Java lineage witness availability negative control must mutate the actual probe call",
  );
  assert.match(
    androidLineageWitnessAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Android Java lineage witness availability probe drift was not detected"\)/u,
    "Android Java lineage witness availability negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    androidLineageWitnessAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Android Java lineage witness availability negative control must not unconditionally pass after run_checks",
  );
  const androidLineageWitnessAppendAvailabilityProbeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-android-lineage-witness-append-availability-probe":'),
    guard.indexOf('if mode == "--negative-control-jvm-pallas-builder-input-guards":'),
  );
  assert.match(
    androidLineageWitnessAppendAvailabilityProbeBranch,
    /nativeLineageWitnessAppendResult\(probe, probe, probe\)[\s\S]*?nativeLineageWitnessAppendResult\(new byte\[0\], new byte\[0\], new byte\[0\]\)[\s\S]*?run_checks\(mutated\)/u,
    "Android Java lineage witness append availability negative control must mutate the actual append probe call",
  );
  assert.match(
    androidLineageWitnessAppendAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Android Java lineage witness append availability probe drift was not detected"\)/u,
    "Android Java lineage witness append availability negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    androidLineageWitnessAppendAvailabilityProbeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Android Java lineage witness append availability negative control must not unconditionally pass after run_checks",
  );
  const nonCsharpPallasBuilderInputGuardBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-non-csharp-pallas-builder-input-guards":'),
    guard.indexOf('if mode == "--negative-control-non-csharp-pallas-builder-native-output-guards":'),
  );
  assert.match(
    nonCsharpPallasBuilderInputGuardBranch,
    /IrohaSwift\/Tests\/IrohaSwiftTests\/KagemushaRecursiveSpendProverTests\.swift[\s\S]*?testRejectsMalformedInputArchivesBeforeBridgeCall[\s\S]*?javascript\/iroha_js\/test\/kagemushaRecursiveSpend\.test\.js[\s\S]*?Kagemusha recursive spend helpers reject malformed Norito request archives before native calls[\s\S]*?python\/iroha_python\/tests\/kagemusha_test\.py[\s\S]*?PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD[\s\S]*?oversized_archive[\s\S]*?valid_archive[\s\S]*?run_checks\(mutated\)/u,
    "non-C# Pallas builder input negative control must mutate Swift, JavaScript, and Python guard tests",
  );
  assert.doesNotMatch(
    nonCsharpPallasBuilderInputGuardBranch,
    /csharp\/README\.md|csharp\//u,
    "non-C# Pallas builder input negative control must leave C# for the Windows follow-up",
  );
  assert.match(
    nonCsharpPallasBuilderInputGuardBranch,
    /missing\s*=\s*\[[\s\S]*?for _target, \(_old, _new, label\) in replacements\.items\(\)[\s\S]*?label not in message[\s\S]*?non-C# Pallas builder input guard drift was not detected for/u,
    "non-C# Pallas builder input negative control must require every mutated SDK label to be reported",
  );
  assert.match(
    nonCsharpPallasBuilderInputGuardBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: non-C# Pallas builder input guard drift was not detected"\)/u,
    "non-C# Pallas builder input negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    nonCsharpPallasBuilderInputGuardBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "non-C# Pallas builder input negative control must not unconditionally pass after run_checks",
  );
  const nonCsharpPallasBuilderNativeOutputGuardBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-non-csharp-pallas-builder-native-output-guards":'),
    guard.indexOf('if mode == "--negative-control-js-lineage-readonly-declarations":'),
  );
  assert.match(
    nonCsharpPallasBuilderNativeOutputGuardBranch,
    /javascript\/iroha_js\/test\/kagemushaRecursiveSpend\.test\.js[\s\S]*?native kagemushaBuildPreviousProofOpenEnvelopesArchive returned invalid Norito archive[\s\S]*?native kagemushaBuildPreviousProofOpenEnvelopesArchive returned unchecked bytes[\s\S]*?python\/iroha_python\/tests\/kagemusha_test\.py[\s\S]*?native\.kagemusha_build_previous_proof_open_envelopes_archive = malformed_one[\s\S]*?native\.kagemusha_build_previous_proof_open_envelopes_archive = empty_payload_one[\s\S]*?run_checks\(mutated\)/u,
    "non-C# Pallas builder native-output negative control must mutate JavaScript and Python guard tests",
  );
  assert.doesNotMatch(
    nonCsharpPallasBuilderNativeOutputGuardBranch,
    /csharp\/README\.md|csharp\//u,
    "non-C# Pallas builder native-output negative control must leave C# for the Windows follow-up",
  );
  assert.match(
    nonCsharpPallasBuilderNativeOutputGuardBranch,
    /missing\s*=\s*\[[\s\S]*?for _target, \(_old, _new, label\) in replacements\.items\(\)[\s\S]*?label not in message[\s\S]*?non-C# Pallas builder native output drift was not detected for/u,
    "non-C# Pallas builder native-output negative control must require every mutated SDK label to be reported",
  );
  assert.match(
    nonCsharpPallasBuilderNativeOutputGuardBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: non-C# Pallas builder native output drift was not detected"\)/u,
    "non-C# Pallas builder native-output negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    nonCsharpPallasBuilderNativeOutputGuardBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "non-C# Pallas builder native-output negative control must not unconditionally pass after run_checks",
  );
  const jsLineageReadonlyDeclarationsBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-lineage-readonly-declarations":'),
    guard.indexOf('if mode == "--negative-control-sdk-archive-input-copy":'),
  );
  assert.match(
    jsLineageReadonlyDeclarationsBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JS lineage key artifact readonly declaration drift was not detected"\s*\)/u,
    "JS lineage key artifact readonly declaration negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsLineageReadonlyDeclarationsBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS lineage key artifact readonly declaration negative control must not unconditionally pass after run_checks",
  );
  const sdkArchiveInputCopyBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-archive-input-copy":'),
    guard.indexOf('if mode == "--negative-control-sdk-lineage-proving-key-copy":'),
  );
  assert.match(
    sdkArchiveInputCopyBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: SDK archive input copy drift was not detected"\)/u,
    "SDK archive input copy negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkArchiveInputCopyBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK archive input copy negative control must not unconditionally pass after run_checks",
  );
  const sdkLineageProvingKeyCopyBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-lineage-proving-key-copy":'),
    guard.indexOf('if mode == "--negative-control-sdk-helper-surface":'),
  );
  assert.match(
    sdkLineageProvingKeyCopyBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: SDK lineage proving key artifact copy drift was not detected"\)/u,
    "SDK lineage proving key artifact copy negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkLineageProvingKeyCopyBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK lineage proving key artifact copy negative control must not unconditionally pass after run_checks",
  );
  const sdkReadmeBoundaryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-readme-boundary":'),
    guard.indexOf('if mode == "--negative-control-sdk-readme-proof-chain-accumulator":'),
  );
  assert.match(
    sdkReadmeBoundaryBranch,
    /IrohaSwift\/README\.md[\s\S]*?java\/iroha_android\/README\.md[\s\S]*?kotlin\/README\.md[\s\S]*?javascript\/iroha_js\/README\.md[\s\S]*?python\/iroha_python\/README\.md[\s\S]*?may rewrite the previous proof archive before native dispatch/u,
    "SDK README boundary negative control must mutate the previous-proof boundary across non-C# READMEs",
  );
  assert.doesNotMatch(
    sdkReadmeBoundaryBranch,
    /csharp\/README\.md/u,
    "SDK README boundary negative control must leave C# for the Windows follow-up",
  );
  assert.match(
    sdkReadmeBoundaryBranch,
    /missing\s*=\s*\[[\s\S]*?for target in replacements[\s\S]*?missing previous-proof opening archive boundary[\s\S]*?SDK README boundary drift was not detected for/u,
    "SDK README boundary negative control must require every non-C# README drift to be reported",
  );
  assert.match(
    sdkReadmeBoundaryBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: SDK README boundary drift was not detected"\)/u,
    "SDK README boundary negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkReadmeBoundaryBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK README boundary negative control must not unconditionally pass after run_checks",
  );
  const sdkReadmeRecursiveCompactUnavailableBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-readme-recursive-compact-unavailable":'),
    guard.indexOf('if mode == "--negative-control-sdk-readme-compact-projection-verifier":'),
  );
  assert.match(
    sdkReadmeRecursiveCompactUnavailableBranch,
    /targets\s*=\s*\([\s\S]*?IrohaSwift\/README\.md[\s\S]*?java\/iroha_android\/README\.md[\s\S]*?kotlin\/README\.md[\s\S]*?javascript\/iroha_js\/README\.md[\s\S]*?python\/iroha_python\/README\.md[\s\S]*?\)[\s\S]*?reserved ABI-7 state[\s\S]*?ABI-7 native state[\s\S]*?run_checks\(mutated\)/u,
    "SDK README recursive compact negative control must mutate the ABI-7 one-hop boundary across non-C# READMEs",
  );
  assert.doesNotMatch(
    sdkReadmeRecursiveCompactUnavailableBranch,
    /csharp\/README\.md/u,
    "SDK README recursive compact negative control must leave C# for the Windows follow-up",
  );
  assert.match(
    sdkReadmeRecursiveCompactUnavailableBranch,
    /missing\s*=\s*\[[\s\S]*?for target in targets[\s\S]*?missing recursive compact ABI-7 boundary[\s\S]*?SDK README recursive compact unavailable drift was not detected for/u,
    "SDK README recursive compact negative control must require every non-C# README drift to be reported",
  );
  assert.match(
    sdkReadmeRecursiveCompactUnavailableBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: SDK README recursive compact unavailable drift was not detected"\)/u,
    "SDK README recursive compact negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkReadmeRecursiveCompactUnavailableBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK README recursive compact negative control must not unconditionally pass after run_checks",
  );
  const sdkReadmeProofChainAccumulatorBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-readme-proof-chain-accumulator":'),
    guard.indexOf('if mode == "--negative-control-sdk-readme-pallas-builder-surface":'),
  );
  assert.match(
    sdkReadmeProofChainAccumulatorBranch,
    /targets\s*=\s*\([\s\S]*?IrohaSwift\/README\.md[\s\S]*?java\/iroha_android\/README\.md[\s\S]*?kotlin\/README\.md[\s\S]*?javascript\/iroha_js\/README\.md[\s\S]*?python\/iroha_python\/README\.md[\s\S]*?\)[\s\S]*?previous recursive proof bytes and per-hop accumulator[\s\S]*?native-owned accumulator digests[\s\S]*?append-boundary[\s\S]*?scalar-projection[\s\S]*?previous\/resulting accumulator digests[\s\S]*?must not derive, supply, or patch accumulator state[\s\S]*?optional SDK metadata[\s\S]*?run_checks\(mutated\)/u,
    "SDK README proof-chain accumulator negative control must mutate the accumulator boundary across non-C# READMEs",
  );
  assert.doesNotMatch(
    sdkReadmeProofChainAccumulatorBranch,
    /csharp\/README\.md/u,
    "SDK README proof-chain accumulator negative control must leave C# for the Windows follow-up",
  );
  assert.match(
    sdkReadmeProofChainAccumulatorBranch,
    /missing\s*=\s*\[[\s\S]*?for target in targets[\s\S]*?missing previous-proof opening archive boundary[\s\S]*?SDK README proof-chain accumulator drift was not detected for/u,
    "SDK README proof-chain accumulator negative control must require every non-C# README drift to be reported",
  );
  assert.match(
    sdkReadmeProofChainAccumulatorBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: SDK README proof-chain accumulator drift was not detected"\)/u,
    "SDK README proof-chain accumulator negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkReadmeProofChainAccumulatorBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK README proof-chain accumulator negative control must not unconditionally pass after run_checks",
  );
  const sdkReadmePallasBuilderBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-readme-pallas-builder-surface":'),
    guard.indexOf('if mode == "--negative-control-offline-doc-native-owned-accumulator-boundary":'),
  );
  assert.match(
    sdkReadmePallasBuilderBranch,
    /IrohaSwift\/README\.md[\s\S]*?buildPallasOpenEnvelopesArchive\(recordBundleArchive:\)[\s\S]*?buildPallasOpenMetadataArchive\(recordBundleArchive:\)[\s\S]*?java\/iroha_android\/README\.md[\s\S]*?buildPreviousProofOpenEnvelopesArchive\(previousBundleArchive\)[\s\S]*?buildPreviousProofOpenMetadataArchive\(previousBundleArchive\)[\s\S]*?kotlin\/README\.md[\s\S]*?javascript\/iroha_js\/README\.md[\s\S]*?python\/iroha_python\/README\.md/u,
    "SDK README Pallas builder negative control must mutate every non-C# README surface",
  );
  assert.match(
    sdkReadmePallasBuilderBranch,
    /csharp\/README\.md[\s\S]*?BuildPallasOpenEnvelopesArchive\(\.\.\.\)[\s\S]*?BuildPreviousProofOpenEnvelopesArchive\(\.\.\.\)/u,
    "SDK README Pallas builder negative control must now cover the C# builder docs",
  );
  assert.match(
    sdkReadmePallasBuilderBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "SDK README Pallas builder negative control must validate the mutated text snapshot",
  );
  assert.match(
    sdkReadmePallasBuilderBranch,
    /missing\s*=\s*\[[\s\S]*?for target in replacements[\s\S]*?missing Pallas open-envelope builder docs[\s\S]*?SDK README Pallas builder drift was not detected for/u,
    "SDK README Pallas builder negative control must require every non-C# README drift to be reported",
  );
  assert.match(
    sdkReadmePallasBuilderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: SDK README Pallas builder drift was not detected"\)/u,
    "SDK README Pallas builder negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkReadmePallasBuilderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK README Pallas builder negative control must not unconditionally pass after run_checks",
  );
  const offlineDocAccumulatorBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-offline-doc-native-owned-accumulator-boundary":'),
    guard.indexOf('if mode == "--negative-control-offline-doc-pallas-builder-surface":'),
  );
  assert.match(
    offlineDocAccumulatorBranch,
    /previous recursive proof bytes and per-hop[\s\S]*?native-owned accumulator digests[\s\S]*?append-boundary[\s\S]*?scalar-projection[\s\S]*?previous\/resulting accumulator digests[\s\S]*?SDKs must not derive, supply[\s\S]*?accumulator state themselves[\s\S]*?SDKs supply accumulator digests as optional metadata[\s\S]*?run_checks\(mutated\)/u,
    "offline Kagemusha doc accumulator negative control must mutate the native-owned accumulator boundary",
  );
  assert.match(
    offlineDocAccumulatorBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: offline Kagemusha accumulator boundary drift was not detected"\)/u,
    "offline Kagemusha doc accumulator negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    offlineDocAccumulatorBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "offline Kagemusha doc accumulator negative control must not unconditionally pass after run_checks",
  );
  const offlineDocPallasBuilderBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-offline-doc-pallas-builder-surface":'),
    guard.indexOf('if mode == "--negative-control-offline-doc-instruction-transaction-surface":'),
  );
  assert.match(
    offlineDocPallasBuilderBranch,
    /Swift, Kotlin\/JVM, Java Android, JavaScript\/Node, Python, and C#[\s\S]*?Swift, Kotlin\/JVM, Java Android, JavaScript\/Node, and Python[\s\S]*?current-hop and previous-proof Pallas open-envelope archive builders[\s\S]*?caller-provided Pallas open-envelope archive metadata[\s\S]*?BuildPallasOpenEnvelopesArchive[\s\S]*?BuildPallasOpenMetadataArchive[\s\S]*?BuildPreviousProofOpenEnvelopesArchive[\s\S]*?BuildPreviousProofOpenMetadataArchive/u,
    "offline Kagemusha doc Pallas negative control must mutate the C#-inclusive SDK boundary and builder surface",
  );
  assert.match(
    offlineDocPallasBuilderBranch,
    /record bundle or previous recursive\\nbundle[\s\S]*?caller-supplied metadata bundle[\s\S]*?native-owned opaque Norito\\nbytes[\s\S]*?inspect and patch the generated archives/u,
    "offline Kagemusha doc Pallas negative control must mutate the opaque native-owned archive boundary",
  );
  assert.match(
    offlineDocPallasBuilderBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "offline Kagemusha doc Pallas negative control must validate the mutated text snapshot",
  );
  assert.match(
    offlineDocPallasBuilderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: offline Kagemusha Pallas builder drift was not detected"\)/u,
    "offline Kagemusha doc Pallas negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    offlineDocPallasBuilderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "offline Kagemusha doc Pallas negative control must not unconditionally pass after run_checks",
  );
  const offlineDocInstructionTransactionBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-offline-doc-instruction-transaction-surface":'),
    guard.indexOf('if mode == "--negative-control-sdk-proof-chain-accumulator-input":'),
  );
  assert.match(
    offlineDocInstructionTransactionBranch,
    /empty, malformed, tampered, or wrong-type[\s\S]*?instruction archives before transaction payload construction[\s\S]*?empty instruction archives before transaction payload construction[\s\S]*?run_checks\(mutated\)/u,
    "offline Kagemusha doc instruction transaction negative control must mutate the archive validation boundary",
  );
  assert.match(
    offlineDocInstructionTransactionBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: offline Kagemusha instruction transaction surface drift was not detected"\s*\)/u,
    "offline Kagemusha doc instruction transaction negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    offlineDocInstructionTransactionBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "offline Kagemusha doc instruction transaction negative control must not unconditionally pass after run_checks",
  );
  const sdkProofChainAccumulatorInputBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-proof-chain-accumulator-input":'),
    guard.indexOf('if mode == "--negative-control-sdk-accumulator-digest-inputs":'),
  );
  assert.match(
    sdkProofChainAccumulatorInputBranch,
    /StaleProofChainDigestInputFixture[\s\S]*?recursiveProofChainDigest[\s\S]*?proofChainDigest[\s\S]*?recursive_proof_chain_digest[\s\S]*?javascript\/iroha_js\/index\.d\.ts/u,
    "SDK proof-chain accumulator input negative control must inject digest parameters across SDKs and TypeScript declarations",
  );
  assert.match(
    sdkProofChainAccumulatorInputBranch,
    /recursiveProofChainDigestV1[\s\S]*?proofChainDigestBytes[\s\S]*?recursiveProofChainDigestBytes[\s\S]*?RecursiveProofChainDigestV1[\s\S]*?recursive_proof_chain_digest_v1[\s\S]*?javascript\/iroha_js\/index\.d\.ts/u,
    "SDK proof-chain accumulator input negative control must exercise suffixed proof-chain digest aliases",
  );
  assert.match(
    guard,
    /r"\\b\[A-Za-z0-9_\]\*recursiveProofChainDigest",[\s\S]*?r"\\b\[A-Za-z0-9_\]\*recursive_proof_chain_digest",[\s\S]*?r"\\b\[A-Za-z0-9_\]\*proofChainDigest",/u,
    "SDK proof-chain accumulator digest scanner must remain suffix-aware",
  );
  assert.doesNotMatch(
    guard,
    /r"\\b\[A-Za-z0-9_\]\*(?:recursiveProofChainDigest|proofChainDigest|recursive_proof_chain_digest)\\b"/u,
    "SDK proof-chain accumulator digest scanner must not reintroduce exact-name-only trailing word boundaries",
  );
  assert.match(
    sdkProofChainAccumulatorInputBranch,
    /mutated\[target\]\s*\+=\s*addition[\s\S]*?run_checks\(mutated\)/u,
    "SDK proof-chain accumulator input negative control must validate the mutated text snapshot",
  );
  assert.match(
    sdkProofChainAccumulatorInputBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: SDK proof-chain accumulator public input drift was not detected"\s*\)/u,
    "SDK proof-chain accumulator input negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkProofChainAccumulatorInputBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK proof-chain accumulator input negative control must not unconditionally pass after run_checks",
  );
  const sdkAccumulatorDigestInputBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-accumulator-digest-inputs":'),
    guard.indexOf('if mode == "--negative-control-sdk-accumulator-boundary-digest-inputs":'),
  );
  assert.match(
    sdkAccumulatorDigestInputBranch,
    /StaleAccumulatorDigestInputFixture[\s\S]*?lineageDigest[\s\S]*?aggregationTranscriptDigest[\s\S]*?fixedWindowTableBaseDigest[\s\S]*?verifierWitnessBatchDigest[\s\S]*?lineage_digest[\s\S]*?aggregation_transcript_digest[\s\S]*?javascript\/iroha_js\/index\.d\.ts/u,
    "SDK accumulator digest input negative control must inject stale digest parameters across SDKs and TypeScript declarations",
  );
  assert.match(
    sdkAccumulatorDigestInputBranch,
    /lineageDigestV1[\s\S]*?aggregationTranscriptDigestBytes[\s\S]*?aggregationTranscriptDigestV1[\s\S]*?verifierWitnessBatchDigestBytes[\s\S]*?fixedWindowTableBaseDigestV1[\s\S]*?lineage_digest_v1[\s\S]*?aggregation_transcript_digest_bytes/u,
    "SDK accumulator digest input negative control must exercise suffixed digest aliases, not only exact digest names",
  );
  assert.match(
    sdkAccumulatorDigestInputBranch,
    /mutated\[target\]\s*\+=\s*addition[\s\S]*?run_checks\(mutated\)/u,
    "SDK accumulator digest input negative control must validate the mutated text snapshot",
  );
  assert.match(
    sdkAccumulatorDigestInputBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: SDK accumulator digest public input drift was not detected"\s*\)/u,
    "SDK accumulator digest input negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkAccumulatorDigestInputBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK accumulator digest input negative control must not unconditionally pass after run_checks",
  );
  const sdkAccumulatorBoundaryDigestInputBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-accumulator-boundary-digest-inputs":'),
    guard.indexOf('if mode == "--negative-control-sdk-readme-availability-surface":'),
  );
  assert.match(
    sdkAccumulatorBoundaryDigestInputBranch,
    /StaleAccumulatorBoundaryDigestInputFixture[\s\S]*?appendBoundaryDigest[\s\S]*?transitionProfileBindingDigest[\s\S]*?appendOpeningPreflightDigest[\s\S]*?fixedWindowTableScheduleDigest[\s\S]*?fixedWindowSharedTableManifestDigest[\s\S]*?recursiveVerifierScalarProjectionDigest[\s\S]*?PreviousAccumulatorDigest[\s\S]*?append_boundary_digest[\s\S]*?resulting_accumulator_digest[\s\S]*?javascript\/iroha_js\/index\.d\.ts/u,
    "SDK accumulator boundary digest input negative control must inject stale boundary digest parameters across SDKs and TypeScript declarations",
  );
  assert.match(
    sdkAccumulatorBoundaryDigestInputBranch,
    /appendBoundaryDigestV1[\s\S]*?transitionProfileBindingDigestBytes[\s\S]*?appendOpeningPreflightDigestV1[\s\S]*?fixedWindowTableScheduleDigestBytes[\s\S]*?fixedWindowSharedTableManifestDigestV1[\s\S]*?recursiveVerifierScalarProjectionDigestBytes[\s\S]*?PreviousAccumulatorDigestBytes[\s\S]*?append_boundary_digest_v1[\s\S]*?resulting_accumulator_digest_bytes/u,
    "SDK accumulator boundary digest input negative control must exercise suffixed boundary digest aliases",
  );
  assert.match(
    guard,
    /r"\\b\[A-Za-z0-9_\]\*lineageDigest",[\s\S]*?r"\\b\[A-Za-z0-9_\]\*transitionProfileBindingDigest\(\?!Domain\)",[\s\S]*?r"\\b\[A-Za-z0-9_\]\*appendBoundaryDigest",/u,
    "SDK accumulator digest scanner must remain suffix-aware while preserving TransitionProfileBindingDigestDomain constants",
  );
  assert.doesNotMatch(
    guard,
    /r"\\b\[A-Za-z0-9_\]\*(?:lineageDigest|appendBoundaryDigest|accumulatorDigest)\\b"/u,
    "SDK accumulator digest scanner must not reintroduce exact-name-only trailing word boundaries",
  );
  assert.match(
    sdkAccumulatorBoundaryDigestInputBranch,
    /mutated\[target\]\s*\+=\s*addition[\s\S]*?run_checks\(mutated\)/u,
    "SDK accumulator boundary digest input negative control must validate the mutated text snapshot",
  );
  assert.match(
    sdkAccumulatorBoundaryDigestInputBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: SDK accumulator boundary digest public input drift was not detected"\s*\)/u,
    "SDK accumulator boundary digest input negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkAccumulatorBoundaryDigestInputBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK accumulator boundary digest input negative control must not unconditionally pass after run_checks",
  );
  const sdkReadmeAvailabilitySurfaceBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-readme-availability-surface":'),
    guard.indexOf('if mode == "--negative-control-sdk-readme-recursive-compact-unavailable":'),
  );
  assert.match(
    sdkReadmeAvailabilitySurfaceBranch,
    /IrohaSwift\/README\.md[\s\S]*?java\/iroha_android\/README\.md[\s\S]*?kotlin\/README\.md[\s\S]*?javascript\/iroha_js\/README\.md[\s\S]*?kagemushaRecursiveSpendLineageAppendBoundary[\s\S]*?python\/iroha_python\/README\.md[\s\S]*?kagemusha_recursive_spend_lineage_append_boundary/u,
    "SDK README availability negative control must mutate the append-boundary helper across non-C# READMEs",
  );
  assert.doesNotMatch(
    sdkReadmeAvailabilitySurfaceBranch,
    /csharp\/README\.md/u,
    "SDK README availability negative control must leave C# for the Windows follow-up",
  );
  assert.match(
    sdkReadmeAvailabilitySurfaceBranch,
    /missing\s*=\s*\[[\s\S]*?for target in replacements[\s\S]*?missing previous-proof opening archive boundary[\s\S]*?SDK README availability surface drift was not detected for/u,
    "SDK README availability negative control must require every non-C# README drift to be reported",
  );
  assert.match(
    sdkReadmeAvailabilitySurfaceBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: SDK README availability surface drift was not detected"\)/u,
    "SDK README availability negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkReadmeAvailabilitySurfaceBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK README availability negative control must not unconditionally pass after run_checks",
  );
  const sdkReadmeCompactProjectionVerifierBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-readme-compact-projection-verifier":'),
    guard.indexOf('if mode == "--negative-control-sdk-readme-stale-future-lineage":'),
  );
  assert.match(
    sdkReadmeCompactProjectionVerifierBranch,
    /IrohaSwift\/README\.md[\s\S]*?verifyRecursiveSpendCompactPaymentTokenProjection\(compactTokenArchive:verifierRecordArchive:blockHeight:\)[\s\S]*?java\/iroha_android\/README\.md[\s\S]*?verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight\(\.\.\.\)[\s\S]*?kotlin\/README\.md[\s\S]*?javascript\/iroha_js\/README\.md[\s\S]*?kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection\(\.\.\.\)[\s\S]*?python\/iroha_python\/README\.md[\s\S]*?kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height\(\.\.\.\)[\s\S]*?run_checks\(mutated\)/u,
    "SDK README compact projection verifier negative control must mutate every non-C# README signature",
  );
  assert.doesNotMatch(
    sdkReadmeCompactProjectionVerifierBranch,
    /csharp\/README\.md/u,
    "SDK README compact projection verifier negative control must leave C# for the Windows follow-up",
  );
  assert.match(
    sdkReadmeCompactProjectionVerifierBranch,
    /missing\s*=\s*\[[\s\S]*?for target in replacements[\s\S]*?missing recursive compact ABI-7 boundary[\s\S]*?SDK README compact projection verifier drift was not detected for/u,
    "SDK README compact projection verifier negative control must require every non-C# README drift to be reported",
  );
  assert.match(
    sdkReadmeCompactProjectionVerifierBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: SDK README compact projection verifier drift was not detected"\)/u,
    "SDK README compact projection verifier negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkReadmeCompactProjectionVerifierBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK README compact projection verifier negative control must not unconditionally pass after run_checks",
  );
  const sdkReadmeStaleFutureLineageBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-readme-stale-future-lineage":'),
    guard.indexOf('if mode == "--negative-control-sdk-readme-native-output-csharp":'),
  );
  assert.match(
    sdkReadmeStaleFutureLineageBranch,
    /IrohaSwift\/README\.md[\s\S]*?append output selector for this release[\s\S]*?Future Reserved-lineage append output selector for this release[\s\S]*?java\/iroha_android\/README\.md[\s\S]*?Reserved-lineage append output is valid only when[\s\S]*?kotlin\/README\.md[\s\S]*?javascript\/iroha_js\/README\.md[\s\S]*?python\/iroha_python\/README\.md[\s\S]*?Future Reserved-lineage append output is valid only when[\s\S]*?run_checks\(mutated\)/u,
    "SDK README stale future-lineage negative control must mutate every non-C# README stale wording surface",
  );
  assert.doesNotMatch(
    sdkReadmeStaleFutureLineageBranch,
    /csharp\/README\.md/u,
    "SDK README stale future-lineage negative control must leave C# for the Windows follow-up",
  );
  assert.match(
    sdkReadmeStaleFutureLineageBranch,
    /missing\s*=\s*\[[\s\S]*?for target in replacements[\s\S]*?still describes Reserved-lineage append output as future[\s\S]*?stale SDK README Reserved-lineage wording was not detected for/u,
    "SDK README stale future-lineage negative control must require every non-C# README drift to be reported",
  );
  assert.match(
    sdkReadmeStaleFutureLineageBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: stale SDK README Reserved-lineage wording was not detected"\)/u,
    "SDK README stale future-lineage negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkReadmeStaleFutureLineageBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK README stale future-lineage negative control must not unconditionally pass after run_checks",
  );
  const rustRecursiveCompactUnavailableClassifierBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-rust-recursive-compact-unavailable-classifier":'),
    guard.indexOf('if mode == "--negative-control-sdk-recursive-compact-unavailable-helper":'),
  );
  assert.match(
    rustRecursiveCompactUnavailableClassifierBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Rust recursive compact unavailable classifier negative control must validate the mutated text snapshot",
  );
  assert.match(
    rustRecursiveCompactUnavailableClassifierBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Rust recursive compact unavailable classifier drift was not detected"\)/u,
    "Rust recursive compact unavailable classifier negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    rustRecursiveCompactUnavailableClassifierBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Rust recursive compact unavailable classifier negative control must not unconditionally pass after run_checks",
  );
  const sdkRecursiveCompactUnavailableHelperBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-recursive-compact-unavailable-helper":'),
    guard.indexOf('if mode == "--negative-control-recursive-compact-verifier-surface":'),
  );
  assert.match(
    sdkRecursiveCompactUnavailableHelperBranch,
    /isKagemushaRecursiveCompactUnavailable\(error\)[\s\S]*?isKagemushaRecursiveCompactMaybeUnavailable\(error\)[\s\S]*?def is_kagemusha_recursive_compact_unavailable[\s\S]*?def is_kagemusha_recursive_compact_maybe_unavailable[\s\S]*?run_checks\(mutated\)/u,
    "SDK recursive compact unavailable helper negative control must mutate JS and Python helper definitions",
  );
  assert.match(
    sdkRecursiveCompactUnavailableHelperBranch,
    /Python recursive compact verifier and Pallas builder surface/u,
    "SDK recursive compact unavailable helper negative control must require the current Python verifier/Pallas label",
  );
  assert.match(
    sdkRecursiveCompactUnavailableHelperBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: SDK recursive compact unavailable helper drift was not detected"\)/u,
    "SDK recursive compact unavailable helper negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    sdkRecursiveCompactUnavailableHelperBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "SDK recursive compact unavailable helper negative control must not unconditionally pass after run_checks",
  );
  assert.match(
    guard,
    /kagemusha_recursive_compact_ffi_rejects_windowed_records_before_unavailable/u,
    "recursive compact verifier surface guard must pin the focused windowed-record bridge test",
  );
  const recursiveCompactVerifierSurfaceBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-recursive-compact-verifier-surface":'),
    guard.indexOf('if mode == "--negative-control-recursive-compact-key-package-arity":'),
  );
  assert.match(
    recursiveCompactVerifierSurfaceBranch,
    /windowed recursive compact verifier records must reject before proving[\s\S]*?windowed recursive compact verifier records may map to unavailable/u,
    "recursive compact verifier surface negative control must mutate the windowed-record hard-fail guard",
  );
  assert.match(
    recursiveCompactVerifierSurfaceBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "recursive compact verifier surface negative control must validate the mutated text snapshot",
  );
  assert.match(
    recursiveCompactVerifierSurfaceBranch,
    /Python recursive compact verifier and Pallas builder surface/u,
    "recursive compact verifier surface negative control must require the current Python verifier/Pallas label",
  );
  const recursiveCompactKeyPackageArityBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-recursive-compact-key-package-arity":'),
    guard.indexOf('if mode == "--negative-control-python-recursive-compact-probe-arity":'),
  );
  assert.match(
    recursiveCompactKeyPackageArityBranch,
    /StaleRecursiveCompactKeyPackageArityFixture[\s\S]*?recursiveCompactKeyArtifactsArchive:\s*Data\s*=\s*Data\(\)[\s\S]*?fun proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes[\s\S]*?public static byte\[\] proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes[\s\S]*?ReadOnlySpan<byte> pallasOpenEnvelopesArchive/u,
    "recursive compact key-package arity negative control must inject stale SDK overloads",
  );
  assert.match(
    recursiveCompactKeyPackageArityBranch,
    /mutated\[target\]\s*\+=\s*addition[\s\S]*?run_checks\(mutated\)/u,
    "recursive compact key-package arity negative control must validate the mutated text snapshot",
  );
  assert.match(
    recursiveCompactKeyPackageArityBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: ABI-7 recursive compact key-package arity drift was not detected"\s*\)/u,
    "recursive compact key-package arity negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    recursiveCompactKeyPackageArityBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "recursive compact key-package arity negative control must not unconditionally pass after run_checks",
  );
  const pythonRecursiveCompactProbeArityBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-python-recursive-compact-probe-arity":'),
    guard.indexOf('if mode == "--negative-control-js-recursive-compact-key-package-dispatch":'),
  );
  assert.match(
    pythonRecursiveCompactProbeArityBranch,
    /stale_prover_probe[\s\S]*?_RECURSIVE_COMPACT_TOKEN_METHOD[\s\S]*?stale_verifier_probe[\s\S]*?_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD/u,
    "Python recursive compact probe arity negative control must mutate prover and verifier probes",
  );
  assert.match(
    pythonRecursiveCompactProbeArityBranch,
    /mutated\[target\]\s*=\s*mutated_text[\s\S]*?run_checks\(mutated\)/u,
    "Python recursive compact probe arity negative control must validate the mutated text snapshot",
  );
  assert.match(
    pythonRecursiveCompactProbeArityBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: Python recursive compact probe arity drift was not detected"\s*\)/u,
    "Python recursive compact probe arity negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    pythonRecursiveCompactProbeArityBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Python recursive compact probe arity negative control must not unconditionally pass after run_checks",
  );
  const jsRecursiveCompactKeyPackageDispatchBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-recursive-compact-key-package-dispatch":'),
    guard.indexOf('if mode == "--negative-control-js-package-dist-recursive-compact-declarations":'),
  );
  assert.match(
    jsRecursiveCompactKeyPackageDispatchBranch,
    /package dist Kagemusha recursive compact requires key packages before native dispatch[\s\S]*?package dist Kagemusha recursive compact allows missing key packages before native dispatch/u,
    "JS recursive compact key-package dispatch negative control must mutate the package-dist test name",
  );
  assert.match(
    jsRecursiveCompactKeyPackageDispatchBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS recursive compact key-package dispatch negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsRecursiveCompactKeyPackageDispatchBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript recursive compact key-package dispatch drift was not detected"\s*\)/u,
    "JS recursive compact key-package dispatch negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsRecursiveCompactKeyPackageDispatchBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS recursive compact key-package dispatch negative control must not unconditionally pass after run_checks",
  );
  const jsPackageDistRecursiveCompactDeclarationsBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-package-dist-recursive-compact-declarations":'),
    guard.indexOf('if mode == "--negative-control-js-package-dist-accumulator-digest-declarations":'),
  );
  assert.match(
    jsPackageDistRecursiveCompactDeclarationsBranch,
    /package declarations expose recursive compact key-package signatures[\s\S]*?package declarations omit recursive compact key-package signatures/u,
    "JS package dist recursive compact declarations negative control must mutate the package-dist test name",
  );
  assert.match(
    jsPackageDistRecursiveCompactDeclarationsBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS package dist recursive compact declarations negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsPackageDistRecursiveCompactDeclarationsBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript package dist recursive compact declaration drift was not detected"\s*\)/u,
    "JS package dist recursive compact declarations negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsPackageDistRecursiveCompactDeclarationsBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS package dist recursive compact declarations negative control must not unconditionally pass after run_checks",
  );
  const jsPackageDistAccumulatorDigestDeclarationsBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-package-dist-accumulator-digest-declarations":'),
    guard.indexOf('if mode == "--negative-control-js-package-dist-accumulator-digest-denylist":'),
  );
  assert.match(
    jsPackageDistAccumulatorDigestDeclarationsBranch,
    /package declarations keep accumulator digests native-owned[\s\S]*?package declarations allow accumulator digest inputs/u,
    "JS package dist accumulator digest declarations negative control must mutate the package-dist test name",
  );
  assert.match(
    jsPackageDistAccumulatorDigestDeclarationsBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS package dist accumulator digest declarations negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsPackageDistAccumulatorDigestDeclarationsBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript package dist accumulator digest declaration drift was not detected"\s*\)/u,
    "JS package dist accumulator digest declarations negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsPackageDistAccumulatorDigestDeclarationsBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS package dist accumulator digest declarations negative control must not unconditionally pass after run_checks",
  );
  const jsPackageDistAccumulatorDigestDenylistBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-package-dist-accumulator-digest-denylist":'),
    guard.indexOf('if mode == "--negative-control-js-package-dist-terminal-accumulator-digest-denylist":'),
  );
  assert.match(
    jsPackageDistAccumulatorDigestDenylistBranch,
    /appendBoundaryDigest\|AppendBoundaryDigest\|append_boundary_digest\|[\s\S]*?""/u,
    "JS package dist accumulator digest denylist negative control must remove a guarded digest token",
  );
  assert.match(
    jsPackageDistAccumulatorDigestDenylistBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS package dist accumulator digest denylist negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsPackageDistAccumulatorDigestDenylistBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript package dist accumulator digest denylist drift was not detected"\s*\)/u,
    "JS package dist accumulator digest denylist negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsPackageDistAccumulatorDigestDenylistBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS package dist accumulator digest denylist negative control must not unconditionally pass after run_checks",
  );
  const jsPackageDistTerminalAccumulatorDigestDenylistBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-package-dist-terminal-accumulator-digest-denylist":'),
    guard.indexOf('if mode == "--negative-control-js-package-dist-prefixed-accumulator-digest-denylist":'),
  );
  assert.match(
    jsPackageDistTerminalAccumulatorDigestDenylistBranch,
    /previousAccumulatorDigest\|PreviousAccumulatorDigest\|previous_accumulator_digest\|resultingAccumulatorDigest\|ResultingAccumulatorDigest\|resulting_accumulator_digest\|[\s\S]*?""/u,
    "JS package dist terminal accumulator digest denylist negative control must remove previous/resulting digest tokens",
  );
  assert.match(
    jsPackageDistTerminalAccumulatorDigestDenylistBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS package dist terminal accumulator digest denylist negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsPackageDistTerminalAccumulatorDigestDenylistBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript package dist terminal accumulator digest denylist drift was not detected"\s*\)/u,
    "JS package dist terminal accumulator digest denylist negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsPackageDistTerminalAccumulatorDigestDenylistBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS package dist terminal accumulator digest denylist negative control must not unconditionally pass after run_checks",
  );
  const jsPackageDistPrefixedAccumulatorDigestDenylistBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-package-dist-prefixed-accumulator-digest-denylist":'),
    guard.indexOf('if mode == "--negative-control-js-package-dist-suffixed-accumulator-digest-denylist":'),
  );
  assert.match(
    jsPackageDistPrefixedAccumulatorDigestDenylistBranch,
    /\\b\[A-Za-z0-9_\]\*\(\?:lineageDigest[\s\S]*?\\b\(\?:lineageDigest/u,
    "JS package dist prefixed accumulator digest denylist negative control must narrow prefixed matching",
  );
  assert.match(
    jsPackageDistPrefixedAccumulatorDigestDenylistBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS package dist prefixed accumulator digest denylist negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsPackageDistPrefixedAccumulatorDigestDenylistBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript package dist prefixed accumulator digest denylist drift was not detected"\s*\)/u,
    "JS package dist prefixed accumulator digest denylist negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsPackageDistPrefixedAccumulatorDigestDenylistBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS package dist prefixed accumulator digest denylist negative control must not unconditionally pass after run_checks",
  );
  const jsPackageDistSuffixedAccumulatorDigestDenylistBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-package-dist-suffixed-accumulator-digest-denylist":'),
    guard.indexOf('if mode == "--negative-control-js-package-dist-declaration-sweep":'),
  );
  assert.match(
    jsPackageDistSuffixedAccumulatorDigestDenylistBranch,
    /accumulator_digest\)\/u;[\s\S]*?accumulator_digest\)\\b\/u;/u,
    "JS package dist suffixed accumulator digest denylist negative control must reintroduce the trailing boundary",
  );
  assert.match(
    jsPackageDistSuffixedAccumulatorDigestDenylistBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS package dist suffixed accumulator digest denylist negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsPackageDistSuffixedAccumulatorDigestDenylistBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript package dist suffixed accumulator digest denylist drift was not detected"\s*\)/u,
    "JS package dist suffixed accumulator digest denylist negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsPackageDistSuffixedAccumulatorDigestDenylistBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS package dist suffixed accumulator digest denylist negative control must not unconditionally pass after run_checks",
  );
  const jsPackageDistDeclarationSweepBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-package-dist-declaration-sweep":'),
    guard.indexOf('if mode == "--negative-control-js-package-dist-nexus-declaration-sweep":'),
  );
  assert.match(
    jsPackageDistDeclarationSweepBranch,
    /connect\.browser\.d\.ts[\s\S]*?""/u,
    "JS package dist declaration sweep negative control must remove a secondary declaration file from the sweep",
  );
  assert.match(
    jsPackageDistDeclarationSweepBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS package dist declaration sweep negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsPackageDistDeclarationSweepBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript package dist declaration sweep drift was not detected"\s*\)/u,
    "JS package dist declaration sweep negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsPackageDistDeclarationSweepBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS package dist declaration sweep negative control must not unconditionally pass after run_checks",
  );
  const jsPackageDistNexusDeclarationSweepBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-package-dist-nexus-declaration-sweep":'),
    guard.indexOf('if mode == "--negative-control-js-package-dist-kotodama-declaration-sweep":'),
  );
  assert.match(
    jsPackageDistNexusDeclarationSweepBranch,
    /nexus-app\.d\.ts[\s\S]*?""/u,
    "JS package dist Nexus declaration sweep negative control must remove nexus-app.d.ts from the sweep",
  );
  assert.match(
    jsPackageDistNexusDeclarationSweepBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS package dist Nexus declaration sweep negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsPackageDistNexusDeclarationSweepBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript package dist Nexus declaration sweep drift was not detected"\s*\)/u,
    "JS package dist Nexus declaration sweep negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsPackageDistNexusDeclarationSweepBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS package dist Nexus declaration sweep negative control must not unconditionally pass after run_checks",
  );
  const jsPackageDistKotodamaDeclarationSweepBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-package-dist-kotodama-declaration-sweep":'),
    guard.indexOf('if mode == "--negative-control-js-dts-recursive-compact-key-package":'),
  );
  assert.match(
    jsPackageDistKotodamaDeclarationSweepBranch,
    /kotodama-compiler\.d\.ts[\s\S]*?""/u,
    "JS package dist Kotodama declaration sweep negative control must remove kotodama-compiler.d.ts from the sweep",
  );
  assert.match(
    jsPackageDistKotodamaDeclarationSweepBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS package dist Kotodama declaration sweep negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsPackageDistKotodamaDeclarationSweepBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript package dist Kotodama declaration sweep drift was not detected"\s*\)/u,
    "JS package dist Kotodama declaration sweep negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsPackageDistKotodamaDeclarationSweepBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS package dist Kotodama declaration sweep negative control must not unconditionally pass after run_checks",
  );
  const jsDtsRecursiveCompactKeyPackageBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-dts-recursive-compact-key-package":'),
    guard.indexOf('if mode == "--negative-control-python-recursive-compact-root-export":'),
  );
  assert.match(
    jsDtsRecursiveCompactKeyPackageBranch,
    /recursiveCompactKeyArtifactsArchive: BinaryLike[\s\S]*?recursiveCompactKeyArtifactsArchive\?: BinaryLike[\s\S]*?recursiveCompactVerifierKeysArchive: BinaryLike[\s\S]*?recursiveCompactVerifierKeysArchive\?: BinaryLike/u,
    "JS TypeScript recursive compact key-package negative control must make declaration parameters optional",
  );
  assert.match(
    jsDtsRecursiveCompactKeyPackageBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS TypeScript recursive compact key-package negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsDtsRecursiveCompactKeyPackageBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript TypeScript recursive compact key-package declaration drift was not detected"\s*\)/u,
    "JS TypeScript recursive compact key-package negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsDtsRecursiveCompactKeyPackageBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS TypeScript recursive compact key-package negative control must not unconditionally pass after run_checks",
  );
  const pythonRecursiveCompactRootExportBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-python-recursive-compact-root-export":'),
    guard.indexOf('if mode == "--negative-control-recursive-spend-compact-projection-surface":'),
  );
  assert.match(
    pythonRecursiveCompactRootExportBranch,
    /python\/iroha_python\/src\/iroha_python\/__init__\.py[\s\S]*?REQUIRED_RECURSIVE_COMPACT_PYTHON_METHODS[\s\S]*?updated\.replace\(f'    "\{method\}",\\n'/u,
    "Python recursive compact root export negative control must remove root export names",
  );
  assert.match(
    pythonRecursiveCompactRootExportBranch,
    /mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "Python recursive compact root export negative control must validate the mutated text snapshot",
  );
  assert.match(
    pythonRecursiveCompactRootExportBranch,
    /Python package recursive compact and Pallas builder re-exports/u,
    "Python recursive compact root export negative control must require the current combined root-export label",
  );
  assert.match(
    pythonRecursiveCompactRootExportBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: Python recursive compact root re-export drift was not detected"\s*\)/u,
    "Python recursive compact root export negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    pythonRecursiveCompactRootExportBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Python recursive compact root export negative control must not unconditionally pass after run_checks",
  );
  const recursiveSpendCompactProjectionSurfaceBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-recursive-spend-compact-projection-surface":'),
    guard.indexOf('if mode == "--negative-control-js-compact-projection-block-height-validation":'),
  );
  assert.match(
    recursiveSpendCompactProjectionSurfaceBranch,
    /javascript\/iroha_js\/src\/crypto\.js[\s\S]*?native bridge ABI 7 with the compact projection symbol[\s\S]*?native bridge ABI 8 with the compact projection symbol[\s\S]*?python\/iroha_python\/src\/iroha_python\/kagemusha\.py[\s\S]*?native bridge ABI 7 with the compact projection symbol[\s\S]*?native bridge ABI 8 with the compact projection symbol[\s\S]*?IrohaSwift\/Sources\/IrohaSwift\/KagemushaRecursiveCompactPaymentTokenProver\.swift[\s\S]*?public static var isProjectionNativeAvailable[\s\S]*?public static var isProjectionNativeUnavailable[\s\S]*?kotlin\/core-jvm\/src\/main\/java\/org\/hyperledger\/iroha\/sdk\/offline\/KagemushaRecursiveCompactPaymentTokenProver\.kt[\s\S]*?fun isProjectionVerifierNativeAvailable\(\): Boolean[\s\S]*?fun isProjectionVerifierNativeUnavailable\(\): Boolean[\s\S]*?java\/iroha_android\/src\/main\/java\/org\/hyperledger\/iroha\/android\/offline\/KagemushaRecursiveCompactPaymentTokenProver\.java[\s\S]*?public static boolean isProjectionVerifierNativeAvailable\(\)[\s\S]*?public static boolean isProjectionVerifierNativeUnavailable\(\)[\s\S]*?run_checks\(mutated_texts\)/u,
    "recursive spend compact projection negative control must mutate JS, Python, Swift, Kotlin, and Android Java projection APIs",
  );
  assert.doesNotMatch(
    recursiveSpendCompactProjectionSurfaceBranch,
    /csharp\/README\.md|csharp\//u,
    "recursive spend compact projection negative control must leave C# for the Windows follow-up",
  );
  assert.match(
    recursiveSpendCompactProjectionSurfaceBranch,
    /expected_labels\s*=\s*\[[\s\S]*?expected_labels\.append\(label\)[\s\S]*?missing\s*=\s*\[label for label in expected_labels if label not in message\][\s\S]*?recursive spend compact projection surface drift was not detected for/u,
    "recursive spend compact projection negative control must require every mutated SDK label to be reported",
  );
  assert.match(
    recursiveSpendCompactProjectionSurfaceBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: recursive spend compact projection surface drift was not detected"\s*\)/u,
    "recursive spend compact projection negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    recursiveSpendCompactProjectionSurfaceBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "recursive spend compact projection negative control must not unconditionally pass after run_checks",
  );
  const jsCompactProjectionBlockHeightValidationBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-compact-projection-block-height-validation":'),
    guard.indexOf('if mode == "--negative-control-python-recursive-spend-compact-projection-root-export":'),
  );
  assert.match(
    jsCompactProjectionBlockHeightValidationBranch,
    /const checkedBlockHeight = normalizeKagemushaBlockHeight\(blockHeight\);[\s\S]*?const checkedBlockHeight = blockHeight;/u,
    "JS compact projection block-height negative control must remove normalized-height dispatch",
  );
  assert.match(
    jsCompactProjectionBlockHeightValidationBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "JS compact projection block-height negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsCompactProjectionBlockHeightValidationBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JavaScript compact projection block-height validation drift was not detected"\s*\)/u,
    "JS compact projection block-height negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsCompactProjectionBlockHeightValidationBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS compact projection block-height negative control must not unconditionally pass after run_checks",
  );
  const pythonRecursiveSpendCompactProjectionRootExportBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-python-recursive-spend-compact-projection-root-export":'),
    guard.indexOf('if mode == "--negative-control-native-bridge-zero-envelope-pallas-guard":'),
  );
  assert.match(
    pythonRecursiveSpendCompactProjectionRootExportBranch,
    /python\/iroha_python\/src\/iroha_python\/__init__\.py[\s\S]*?method\s*=\s*"kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"[\s\S]*?replace\(f'    "\{method\}",\\n'/u,
    "Python recursive spend compact projection root export negative control must remove the at-height root export",
  );
  assert.match(
    pythonRecursiveSpendCompactProjectionRootExportBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Python recursive spend compact projection root export negative control must validate the mutated text snapshot",
  );
  assert.match(
    pythonRecursiveSpendCompactProjectionRootExportBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: Python recursive spend compact projection root export drift was not detected"\s*\)/u,
    "Python recursive spend compact projection root export negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    pythonRecursiveSpendCompactProjectionRootExportBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Python recursive spend compact projection root export negative control must not unconditionally pass after run_checks",
  );
  const kagemushaAbiProbeBoundsBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-kagemusha-abi-probe-bounds":'),
    guard.indexOf('if mode == "--negative-control-kagemusha-probe-rejection-shape":'),
  );
  assert.match(
    kagemushaAbiProbeBoundsBranch,
    /javascript\/iroha_js\/src\/crypto\.js[\s\S]*?version <= KAGEMUSHA_MAX_NATIVE_BRIDGE_ABI_VERSION[\s\S]*?version <= Number\.MAX_SAFE_INTEGER[\s\S]*?javascript\/iroha_js\/dist\/crypto\.js[\s\S]*?python\/iroha_python\/src\/iroha_python\/kagemusha\.py[\s\S]*?version > KAGEMUSHA_MAX_NATIVE_BRIDGE_ABI_VERSION[\s\S]*?version > 10\*\*100[\s\S]*?run_checks\(mutated\)/u,
    "Kagemusha ABI probe bounds negative control must mutate JS source, JS dist, and Python bounds",
  );
  assert.doesNotMatch(
    kagemushaAbiProbeBoundsBranch,
    /csharp\/README\.md|csharp\//u,
    "Kagemusha ABI probe bounds negative control must leave C# for the Windows follow-up",
  );
  assert.match(
    kagemushaAbiProbeBoundsBranch,
    /missing\s*=\s*\[label for label in expected_labels if label not in message\][\s\S]*?Kagemusha ABI probe bounds drift was not detected for/u,
    "Kagemusha ABI probe bounds negative control must require every mutated non-C# label to be reported",
  );
  assert.match(
    kagemushaAbiProbeBoundsBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Kagemusha ABI probe bounds drift was not detected"\)/u,
    "Kagemusha ABI probe bounds negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    kagemushaAbiProbeBoundsBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Kagemusha ABI probe bounds negative control must not unconditionally pass after run_checks",
  );
  const kagemushaProbeRejectionShapeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-kagemusha-probe-rejection-shape":'),
    guard.indexOf("if mode:\n    raise SystemExit"),
  );
  assertContainsAll(
    kagemushaProbeRejectionShapeBranch,
    [
      "javascript/iroha_js/src/crypto.js",
      "javascript/iroha_js/dist/crypto.js",
      "python/iroha_python/src/iroha_python/kagemusha.py",
      '"/\\\\b(?:archive|Norito|probe)\\\\b/i.test(error.message)"',
      "'(\"archive\", \"norito\", \"probe\")'",
      "Python recursive compact verifier and Pallas builder surface",
      "run_checks(mutated)",
    ],
    "Kagemusha probe rejection shape negative control must mutate JS source, JS dist, and Python probe classifiers",
  );
  assert.doesNotMatch(
    kagemushaProbeRejectionShapeBranch,
    /csharp\/README\.md|csharp\//u,
    "Kagemusha probe rejection shape negative control must leave C# for the Windows follow-up",
  );
  assert.match(
    kagemushaProbeRejectionShapeBranch,
    /missing\s*=\s*\[label for label in expected_labels if label not in message\][\s\S]*?Kagemusha probe rejection shape drift was not detected for/u,
    "Kagemusha probe rejection shape negative control must require every mutated non-C# label to be reported",
  );
  assert.match(
    kagemushaProbeRejectionShapeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Kagemusha probe rejection shape drift was not detected"\)/u,
    "Kagemusha probe rejection shape negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    kagemushaProbeRejectionShapeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Kagemusha probe rejection shape negative control must not unconditionally pass after run_checks",
  );
  const jvmRecursiveCompactVerifierAvailabilityBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-jvm-recursive-compact-verifier-availability":'),
    guard.indexOf('if mode == "--negative-control-jvm-recursive-compact-shape-classifier":'),
  );
  assert.match(
    jvmRecursiveCompactVerifierAvailabilityBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "JVM recursive compact verifier availability negative control must validate the mutated text snapshot",
  );
  assert.match(
    jvmRecursiveCompactVerifierAvailabilityBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JVM recursive compact verifier availability drift was not detected"\)/u,
    "JVM recursive compact verifier availability negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jvmRecursiveCompactVerifierAvailabilityBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JVM recursive compact verifier availability negative control must not unconditionally pass after run_checks",
  );
  const mobileNativeOutputHeaderBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-recursive-spend-native-output-headers":'),
    guard.indexOf('if mode == "--negative-control-mobile-privacy-production-gate-exactness":'),
  );
  assert.match(
    mobileNativeOutputHeaderBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile native output header negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobileNativeOutputHeaderBranch,
    /invalidFieldBitset\[39\] = 0x20[\s\S]*?invalidFieldBitset\[39\] = 0x06/u,
    "mobile native output header negative control must mutate malformed-header coverage",
  );
  assert.match(
    mobileNativeOutputHeaderBranch,
    /Kotlin recursive spend native output Norito guard tests[\s\S]*?Android Java recursive spend native output Norito guard tests/u,
    "mobile native output header negative control must require both mobile guard labels",
  );
  assert.match(
    mobileNativeOutputHeaderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: mobile native output header drift was not detected"\)/u,
    "mobile native output header negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobileNativeOutputHeaderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile native output header negative control must not unconditionally pass after run_checks",
  );
  const mobilePrivacyProductionGateBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-privacy-production-gate-exactness":'),
    guard.indexOf('if mode == "--negative-control-mobile-privacy-audit-hash-uniqueness":'),
  );
  assert.match(
    mobilePrivacyProductionGateBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[swift_bridge\]\s*=\s*swift_mutated[\s\S]*?mutated_texts\[kotlin_bridge\]\s*=\s*kotlin_mutated[\s\S]*?mutated_texts\[java_bridge\]\s*=\s*java_mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile privacy production-gate negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobilePrivacyProductionGateBranch,
    /rows\.contains\(where: \{ !nativeCapabilityRowIsExact\(\$0\) \}\)[\s\S]*?rows\.contains\(where: \{ \$0\.productionGate\.version != version \}\)[\s\S]*?rows\.any \{ !nativeCapabilityRowIsExact\(it\) \}[\s\S]*?rows\.any \{ it\.productionGate\.version != PRODUCTION_GATE_VERSION \}[\s\S]*?if \(!nativeCapabilityRowIsExact\(row\)\)[\s\S]*?if \(!PRODUCTION_GATE_VERSION\.equals\(row\.productionGate\.version\)\)/u,
    "mobile privacy production-gate negative control must mutate exact-row checks to version-only checks",
  );
  assert.match(
    mobilePrivacyProductionGateBranch,
    /Swift privacy production-gate exactness[\s\S]*?Kotlin privacy production-gate exactness[\s\S]*?Android Java privacy production-gate exactness/u,
    "mobile privacy production-gate negative control must require every mobile exactness label",
  );
  assert.match(
    mobilePrivacyProductionGateBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: mobile privacy production-gate exactness drift was not detected"\)/u,
    "mobile privacy production-gate negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobilePrivacyProductionGateBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile privacy production-gate negative control must not unconditionally pass after run_checks",
  );
  const mobilePrivacyAuditHashUniquenessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-privacy-audit-hash-uniqueness":'),
    guard.indexOf('if mode == "--negative-control-mobile-privacy-localnet-lifecycle-audit":'),
  );
  assert.match(
    mobilePrivacyAuditHashUniquenessBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?for target, old, new, extra_old, extra_new, label in mutations:[\s\S]*?mutated_texts\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile privacy audit-hash uniqueness negative control must validate every mutated text snapshot",
  );
  assert.match(
    mobilePrivacyAuditHashUniquenessBranch,
    /var seenHashes = Set<String>\(\)[\s\S]*?seenHashes\.insert\(value\)\.inserted[\s\S]*?val seenHashes = mutableSetOf<String>\(\)[\s\S]*?seenHashes\.add\(value\)[\s\S]*?final Set<String> seenHashes = new HashSet<>\(\);[\s\S]*?seenHashes\.add\(value\)/u,
    "mobile privacy audit-hash uniqueness negative control must mutate every non-C# hash uniqueness guard",
  );
  assert.match(
    mobilePrivacyAuditHashUniquenessBranch,
    /Swift privacy production-gate exactness[\s\S]*?Swift privacy production-gate exactness tests[\s\S]*?Kotlin privacy production-gate exactness[\s\S]*?Kotlin privacy production-gate exactness tests[\s\S]*?Android Java privacy production-gate exactness[\s\S]*?Android Java privacy production-gate exactness tests/u,
    "mobile privacy audit-hash uniqueness negative control must require every non-C# source and test label",
  );
  assert.match(
    mobilePrivacyAuditHashUniquenessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: mobile privacy audit hash uniqueness drift was not detected"\)/u,
    "mobile privacy audit-hash uniqueness negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobilePrivacyAuditHashUniquenessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile privacy audit-hash uniqueness negative control must not unconditionally pass after run_checks",
  );
  const mobilePrivacyLocalnetLifecycleBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-privacy-localnet-lifecycle-audit":'),
    guard.indexOf('if mode == "--negative-control-public-privacy-localnet-lifecycle-catalog":'),
  );
  assert.match(
    mobilePrivacyLocalnetLifecycleBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile privacy localnet lifecycle negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobilePrivacyLocalnetLifecycleBranch,
    /localnet_lifecycle_redeem_tx_hash:[\s\S]*?localnet_lifecycle_generic_redeem_tx_hash:/u,
    "mobile privacy localnet lifecycle negative control must mutate a required localnet audit field",
  );
  assert.match(
    mobilePrivacyLocalnetLifecycleBranch,
    /Swift privacy production-gate exactness[\s\S]*?Swift privacy production-gate exactness tests[\s\S]*?Kotlin privacy production-gate exactness[\s\S]*?Kotlin privacy production-gate exactness tests[\s\S]*?Android Java privacy production-gate exactness[\s\S]*?Android Java privacy production-gate exactness tests/u,
    "mobile privacy localnet lifecycle negative control must require every non-C# source and test label",
  );
  assert.match(
    mobilePrivacyLocalnetLifecycleBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: mobile privacy localnet lifecycle audit drift was not detected"\)/u,
    "mobile privacy localnet lifecycle negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobilePrivacyLocalnetLifecycleBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile privacy localnet lifecycle negative control must not unconditionally pass after run_checks",
  );
  const publicPrivacyLocalnetLifecycleBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-public-privacy-localnet-lifecycle-catalog":'),
    guard.indexOf('if mode == "--negative-control-public-privacy-sdk-export-review-scope-evidence":'),
  );
  assert.match(
    publicPrivacyLocalnetLifecycleBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "public privacy localnet lifecycle negative control must validate the mutated text snapshot",
  );
  assert.match(
    publicPrivacyLocalnetLifecycleBranch,
    /const lifecycleRedeemTxHash = evidenceHashUri\(value\.lifecycle_redeem_tx_hash\)[\s\S]*?const lifecycleRedeemTxHash = evidenceHashUri\(value\.lifecycle_generic_redeem_tx_hash\)[\s\S]*?reused localnet lifecycle hash cross scheme[\s\S]*?reused localnet generic lifecycle hash cross scheme[\s\S]*?reused-localnet-lifecycle-hash-cross-scheme[\s\S]*?reused-localnet-generic-lifecycle-hash-cross-scheme[\s\S]*?privacy_production_ready_gate_hashes_are_distinct\(row\)[\s\S]*?&& true[\s\S]*?reused fuzz localnet artifact hash[\s\S]*?reused generic localnet artifact hash/u,
    "public privacy localnet lifecycle negative control must mutate required JS/Python lifecycle markers",
  );
  assert.match(
    publicPrivacyLocalnetLifecycleBranch,
    /javascript\/iroha_js\/src\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?javascript\/iroha_js\/dist\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?JavaScript privacy localnet lifecycle catalog tests[\s\S]*?Python privacy catalog must require full localnet lifecycle evidence[\s\S]*?Python privacy catalog tests must reject malformed localnet lifecycle evidence[\s\S]*?Python native privacy production evidence ready-gate hash distinctness[\s\S]*?Python native privacy production evidence reused hash tests[\s\S]*?Python native privacy production gate invariant reused hash test/u,
    "public privacy localnet lifecycle negative control must require source, dist, and test labels",
  );
  assert.match(
    publicPrivacyLocalnetLifecycleBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: public privacy localnet lifecycle catalog drift was not detected"\)/u,
    "public privacy localnet lifecycle negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    publicPrivacyLocalnetLifecycleBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "public privacy localnet lifecycle negative control must not unconditionally pass after run_checks",
  );
  const publicPrivacySdkExportScopeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-public-privacy-sdk-export-review-scope-evidence":'),
    guard.indexOf('if mode == "--negative-control-public-privacy-zero-hash-evidence":'),
  );
  assert.match(
    publicPrivacySdkExportScopeBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "public privacy SDK export/review-scope negative control must validate the mutated text snapshot",
  );
  assert.match(
    publicPrivacySdkExportScopeBranch,
    /const sdkExports = evidenceSdkExports\(source\.sdk_exports, sdkEntrypoints\)[\s\S]*?Object\.fromEntries\(PRIVACY_PRODUCTION_SDK_ENTRYPOINT_SURFACES\.map[\s\S]*?missing SDK exports[\s\S]*?missing ignored SDK exports[\s\S]*?export type PrivacyProductionGateEvidence = Readonly<[\s\S]*?export type PrivacyProductionGateArtifactEvidence = Readonly<[\s\S]*?const artifact = declarationInterface[\s\S]*?const artifactIgnored = declarationInterface[\s\S]*?function privacyPackageProductionEvidenceManifest\([\s\S]*?function privacyPackageProductionEvidenceManifestIgnored\(/u,
    "public privacy SDK export/review-scope negative control must mutate parser, test, and declaration markers",
  );
  assert.match(
    publicPrivacySdkExportScopeBranch,
    /javascript\/iroha_js\/src\/privacyAlgorithms\.js privacy SDK export\/review-scope evidence[\s\S]*?javascript\/iroha_js\/dist\/privacyAlgorithms\.js privacy SDK export\/review-scope evidence[\s\S]*?JavaScript privacy SDK export\/review-scope evidence tests[\s\S]*?JavaScript privacy production evidence declarations[\s\S]*?JavaScript package privacy production evidence declaration tests[\s\S]*?JavaScript package privacy production evidence runtime tests/u,
    "public privacy SDK export/review-scope negative control must require source, dist, declaration, runtime, and test labels",
  );
  assert.match(
    publicPrivacySdkExportScopeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: public privacy SDK export\/review-scope drift was not detected"\s*\)/u,
    "public privacy SDK export/review-scope negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    publicPrivacySdkExportScopeBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "public privacy SDK export/review-scope negative control must not unconditionally pass after run_checks",
  );
  const publicPrivacyZeroHashBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-public-privacy-zero-hash-evidence":'),
    guard.indexOf('if mode == "--negative-control-public-privacy-repeated-hash-evidence":'),
  );
  assert.match(
    publicPrivacyZeroHashBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "public privacy zero-hash negative control must validate the mutated text snapshot",
  );
  assert.match(
    publicPrivacyZeroHashBranch,
    /digest === "0"\.repeat\(64\)[\s\S]*?placeholder localnet lifecycle hash[\s\S]*?digest == "0" \* 64[\s\S]*?placeholder-localnet-lifecycle-hash/u,
    "public privacy zero-hash negative control must mutate JS/Python zero-hash guards and tests",
  );
  assert.match(
    publicPrivacyZeroHashBranch,
    /javascript\/iroha_js\/src\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?javascript\/iroha_js\/dist\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?JavaScript privacy localnet lifecycle catalog tests[\s\S]*?Python privacy catalog must require full localnet lifecycle evidence[\s\S]*?Python privacy catalog tests must reject malformed localnet lifecycle evidence/u,
    "public privacy zero-hash negative control must require source, dist, and test labels",
  );
  assert.match(
    publicPrivacyZeroHashBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: public privacy zero-hash evidence drift was not detected"\)/u,
    "public privacy zero-hash negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    publicPrivacyZeroHashBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "public privacy zero-hash negative control must not unconditionally pass after run_checks",
  );
  const publicPrivacyRepeatedHashBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-public-privacy-repeated-hash-evidence":'),
    guard.indexOf('if mode == "--negative-control-public-privacy-zero-signature-evidence":'),
  );
  assert.match(
    publicPrivacyRepeatedHashBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "public privacy repeated-hash negative control must validate the mutated text snapshot",
  );
  assert.match(
    publicPrivacyRepeatedHashBranch,
    /new Set\(digest\)\.size === 1[\s\S]*?uniform-review-artifact-digest-marker[\s\S]*?len\(set\(digest\)\) == 1[\s\S]*?uniform-review-artifact-digest-marker/u,
    "public privacy repeated-hash negative control must mutate JS/Python repeated-hash guards and tests",
  );
  assert.match(
    publicPrivacyRepeatedHashBranch,
    /javascript\/iroha_js\/src\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?javascript\/iroha_js\/dist\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?JavaScript privacy localnet lifecycle catalog tests[\s\S]*?Python privacy catalog must require full localnet lifecycle evidence[\s\S]*?Python privacy catalog tests must reject malformed localnet lifecycle evidence/u,
    "public privacy repeated-hash negative control must require source, dist, and test labels",
  );
  assert.match(
    publicPrivacyRepeatedHashBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: public privacy repeated-hash evidence drift was not detected"\)/u,
    "public privacy repeated-hash negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    publicPrivacyRepeatedHashBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "public privacy repeated-hash negative control must not unconditionally pass after run_checks",
  );
  const publicPrivacyZeroSignatureBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-public-privacy-zero-signature-evidence":'),
    guard.indexOf('if mode == "--negative-control-public-privacy-repeated-signature-evidence":'),
  );
  assert.match(
    publicPrivacyZeroSignatureBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "public privacy zero-signature negative control must validate the mutated text snapshot",
  );
  assert.match(
    publicPrivacyZeroSignatureBranch,
    /signatureBody === "0"\.repeat\(128\)[\s\S]*?placeholder review artifact signature[\s\S]*?signature_body == "0" \* 128[\s\S]*?placeholder-review-artifact-signature/u,
    "public privacy zero-signature negative control must mutate JS/Python zero-signature guards and tests",
  );
  assert.match(
    publicPrivacyZeroSignatureBranch,
    /javascript\/iroha_js\/src\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?javascript\/iroha_js\/dist\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?JavaScript privacy localnet lifecycle catalog tests[\s\S]*?Python privacy catalog must require full localnet lifecycle evidence[\s\S]*?Python privacy catalog tests must reject malformed localnet lifecycle evidence/u,
    "public privacy zero-signature negative control must require source, dist, and test labels",
  );
  assert.match(
    publicPrivacyZeroSignatureBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: public privacy zero-signature evidence drift was not detected"\)/u,
    "public privacy zero-signature negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    publicPrivacyZeroSignatureBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "public privacy zero-signature negative control must not unconditionally pass after run_checks",
  );
  const publicPrivacyRepeatedSignatureBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-public-privacy-repeated-signature-evidence":'),
    guard.indexOf('if mode == "--negative-control-public-privacy-reviewer-identity-evidence":'),
  );
  assert.match(
    publicPrivacyRepeatedSignatureBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "public privacy repeated-signature negative control must validate the mutated text snapshot",
  );
  assert.match(
    publicPrivacyRepeatedSignatureBranch,
    /new Set\(signatureBody\)\.size === 1[\s\S]*?placeholder review artifact sig marker[\s\S]*?len\(set\(signature_body\)\) == 1[\s\S]*?placeholder-review-artifact-sig-marker/u,
    "public privacy repeated-signature negative control must mutate JS/Python repeated-signature guards and tests",
  );
  assert.match(
    publicPrivacyRepeatedSignatureBranch,
    /javascript\/iroha_js\/src\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?javascript\/iroha_js\/dist\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?JavaScript privacy localnet lifecycle catalog tests[\s\S]*?Python privacy catalog must require full localnet lifecycle evidence[\s\S]*?Python privacy catalog tests must reject malformed localnet lifecycle evidence/u,
    "public privacy repeated-signature negative control must require source, dist, and test labels",
  );
  assert.match(
    publicPrivacyRepeatedSignatureBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: public privacy repeated-signature evidence drift was not detected"\)/u,
    "public privacy repeated-signature negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    publicPrivacyRepeatedSignatureBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "public privacy repeated-signature negative control must not unconditionally pass after run_checks",
  );
  const publicPrivacyReviewerIdentityBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-public-privacy-reviewer-identity-evidence":'),
    guard.indexOf('if mode == "--negative-control-public-privacy-artifact-label-evidence":'),
  );
  assert.match(
    publicPrivacyReviewerIdentityBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "public privacy reviewer-identity negative control must validate the mutated text snapshot",
  );
  assert.match(
    publicPrivacyReviewerIdentityBranch,
    /reviewerIdentityValue\(source\.reviewer_identity\)[\s\S]*?evidenceTextValue\(source\.reviewer_identity, 160\)[\s\S]*?placeholder reviewer marker[\s\S]*?_privacy_evidence_reviewer_identity[\s\S]*?_privacy_evidence_text_value[\s\S]*?placeholder-reviewer-marker/u,
    "public privacy reviewer-identity negative control must mutate JS/Python reviewer identity guards and tests",
  );
  assert.match(
    publicPrivacyReviewerIdentityBranch,
    /javascript\/iroha_js\/src\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?javascript\/iroha_js\/dist\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?JavaScript privacy localnet lifecycle catalog tests[\s\S]*?Python privacy catalog must require full localnet lifecycle evidence[\s\S]*?Python privacy catalog tests must reject malformed localnet lifecycle evidence/u,
    "public privacy reviewer-identity negative control must require source, dist, and test labels",
  );
  assert.match(
    publicPrivacyReviewerIdentityBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: public privacy reviewer-identity evidence drift was not detected"\)/u,
    "public privacy reviewer-identity negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    publicPrivacyReviewerIdentityBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "public privacy reviewer-identity negative control must not unconditionally pass after run_checks",
  );
  const publicPrivacyArtifactLabelBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-public-privacy-artifact-label-evidence":'),
    guard.indexOf('if mode == "--negative-control-public-privacy-duplicate-row-evidence":'),
  );
  assert.match(
    publicPrivacyArtifactLabelBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "public privacy artifact-label negative control must validate the mutated text snapshot",
  );
  assert.match(
    publicPrivacyArtifactLabelBranch,
    /artifactLabelValue\(value\.label\)[\s\S]*?evidenceTextValue\(value\.label, 160\)[\s\S]*?placeholder production gate artifact marker[\s\S]*?_privacy_evidence_artifact_label[\s\S]*?_privacy_evidence_text_value[\s\S]*?placeholder-production-gate-artifact-marker/u,
    "public privacy artifact-label negative control must mutate JS/Python artifact label guards and tests",
  );
  assert.match(
    publicPrivacyArtifactLabelBranch,
    /javascript\/iroha_js\/src\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?javascript\/iroha_js\/dist\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?JavaScript privacy localnet lifecycle catalog tests[\s\S]*?Python privacy catalog must require full localnet lifecycle evidence[\s\S]*?Python privacy catalog tests must reject malformed localnet lifecycle evidence/u,
    "public privacy artifact-label negative control must require source, dist, and test labels",
  );
  assert.match(
    publicPrivacyArtifactLabelBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: public privacy artifact-label evidence drift was not detected"\)/u,
    "public privacy artifact-label negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    publicPrivacyArtifactLabelBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "public privacy artifact-label negative control must not unconditionally pass after run_checks",
  );
  const publicPrivacyDuplicateRowBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-public-privacy-duplicate-row-evidence":'),
    guard.indexOf('if mode == "--negative-control-public-privacy-deterministic-test-artifact":'),
  );
  assert.match(
    publicPrivacyDuplicateRowBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "public privacy duplicate-row negative control must validate the mutated text snapshot",
  );
  assert.match(
    publicPrivacyDuplicateRowBranch,
    /Object\.hasOwn\(rows, rowId\)[\s\S]*?accept duplicate internal review evidence rows[\s\S]*?if row_id in rows:[\s\S]*?test_privacy_catalog_accepts_duplicate_internal_review_evidence_rows/u,
    "public privacy duplicate-row negative control must mutate JS/Python duplicate-row guards and tests",
  );
  assert.match(
    publicPrivacyDuplicateRowBranch,
    /javascript\/iroha_js\/src\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?javascript\/iroha_js\/dist\/privacyAlgorithms\.js privacy localnet lifecycle catalog[\s\S]*?JavaScript privacy localnet lifecycle catalog tests[\s\S]*?Python privacy catalog must require full localnet lifecycle evidence[\s\S]*?Python privacy catalog tests must reject malformed localnet lifecycle evidence/u,
    "public privacy duplicate-row negative control must require source, dist, and test labels",
  );
  assert.match(
    publicPrivacyDuplicateRowBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: public privacy duplicate-row evidence drift was not detected"\)/u,
    "public privacy duplicate-row negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    publicPrivacyDuplicateRowBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "public privacy duplicate-row negative control must not unconditionally pass after run_checks",
  );
  const publicPrivacyDeterministicTestArtifactBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-public-privacy-deterministic-test-artifact":'),
    guard.indexOf('if mode == "--negative-control-mobile-zk-merkle-provider-adversarial-coverage":'),
  );
  assert.match(
    publicPrivacyDeterministicTestArtifactBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "public privacy deterministic test artifact negative control must validate the mutated text snapshot",
  );
  assert.match(
    publicPrivacyDeterministicTestArtifactBranch,
    /createHash\("sha256"\)\.update\(label\)\.digest\("hex"\)[\s\S]*?truncated label hex[\s\S]*?hashlib\.sha256\(label\.encode\("utf-8"\)\)\.hexdigest\(\)[\s\S]*?helper_uses_hash/u,
    "public privacy deterministic test artifact negative control must mutate JS/Python helper digests and tests",
  );
  assert.match(
    publicPrivacyDeterministicTestArtifactBranch,
    /JavaScript privacy localnet lifecycle catalog tests[\s\S]*?Python privacy catalog tests must reject malformed localnet lifecycle evidence/u,
    "public privacy deterministic test artifact negative control must require JS and Python test labels",
  );
  assert.match(
    publicPrivacyDeterministicTestArtifactBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: public privacy deterministic test artifact drift was not detected"\)/u,
    "public privacy deterministic test artifact negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    publicPrivacyDeterministicTestArtifactBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "public privacy deterministic test artifact negative control must not unconditionally pass after run_checks",
  );
  const mobileZkMerkleProviderBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-zk-merkle-provider-adversarial-coverage":'),
    guard.indexOf('if mode == "--negative-control-mobile-zk-torii-parser-shape-coverage":'),
  );
  assert.match(
    mobileZkMerkleProviderBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile ZK Merkle provider negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobileZkMerkleProviderBranch,
    /toriiProviderRejectsPathCountDriftAndReorderedNodeResponses[\s\S]*?toriiProviderAllowsPathCountDriftAndReorderedNodeResponses/u,
    "mobile ZK Merkle provider negative control must mutate adversarial test coverage",
  );
  assert.match(
    mobileZkMerkleProviderBranch,
    /Kotlin ZK Merkle Torii provider adversarial tests[\s\S]*?Android Java ZK Merkle Torii provider adversarial tests/u,
    "mobile ZK Merkle provider negative control must require both mobile adversarial labels",
  );
  assert.match(
    mobileZkMerkleProviderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: mobile ZK Merkle provider adversarial coverage drift was not detected"\s*\)/u,
    "mobile ZK Merkle provider negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobileZkMerkleProviderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile ZK Merkle provider negative control must not unconditionally pass after run_checks",
  );
  const mobileZkToriiParserBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-zk-torii-parser-shape-coverage":'),
    guard.indexOf('if mode == "--negative-control-mobile-confidential-note-coverage":'),
  );
  assert.match(
    mobileZkToriiParserBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile ZK Torii parser shape negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobileZkToriiParserBranch,
    /rootsAndMerklePathsRejectOverflowDuplicateKeysAndInconsistentShape[\s\S]*?rootsAndMerklePathsAllowOverflowDuplicateKeysAndInconsistentShape[\s\S]*?merklePathParserRejectsDuplicateKeysBeforeLastValueWins[\s\S]*?merklePathParserAllowsDuplicateKeysBeforeLastValueWins/u,
    "mobile ZK Torii parser shape negative control must mutate parser shape coverage",
  );
  assert.match(
    mobileZkToriiParserBranch,
    /Kotlin ZK Torii parser shape tests[\s\S]*?Android Java ZK Torii parser shape tests/u,
    "mobile ZK Torii parser shape negative control must require both mobile parser labels",
  );
  assert.match(
    mobileZkToriiParserBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: mobile ZK Torii parser shape coverage drift was not detected"\)/u,
    "mobile ZK Torii parser shape negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobileZkToriiParserBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile ZK Torii parser shape negative control must not unconditionally pass after run_checks",
  );
  const mobileConfidentialNoteBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-confidential-note-coverage":'),
    guard.indexOf('if mode == "--negative-control-mobile-offline-readiness-coverage":'),
  );
  assert.match(
    mobileConfidentialNoteBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile confidential note negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobileConfidentialNoteBranch,
    /confidentialEncryptedPayloadIsStrictAndDefensive[\s\S]*?confidentialEncryptedPayloadAllowsAmbiguousInputs[\s\S]*?derivesRustConfidentialV2Vectors[\s\S]*?derivesDriftedConfidentialV2Vectors[\s\S]*?encryptsAndDecryptsPlaintextContract[\s\S]*?encryptsAndDecryptsDriftedPlaintextContract/u,
    "mobile confidential note negative control must mutate payload and note coverage",
  );
  assert.match(
    mobileConfidentialNoteBranch,
    /Kotlin confidential encrypted payload tests[\s\S]*?Android Java confidential encrypted payload tests[\s\S]*?Kotlin confidential note contract tests[\s\S]*?Android Java confidential note contract tests/u,
    "mobile confidential note negative control must require both mobile payload and note labels",
  );
  assert.match(
    mobileConfidentialNoteBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: mobile confidential note coverage drift was not detected"\)/u,
    "mobile confidential note negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobileConfidentialNoteBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile confidential note negative control must not unconditionally pass after run_checks",
  );
  const mobileOfflineReadinessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-offline-readiness-coverage":'),
    guard.indexOf('if mode == "--negative-control-kotlin-offline-cash-settlement-coverage":'),
  );
  assert.match(
    mobileOfflineReadinessBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile offline readiness negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobileOfflineReadinessBranch,
    /readinessRejectsMalformedPresentAliasValues[\s\S]*?readinessAllowsMalformedPresentAliasValues[\s\S]*?v2ReadinessUsesCanonicalGetPathAndParsesResponse[\s\S]*?v2ReadinessUsesNoncanonicalGetPathAndParsesResponse[\s\S]*?rejectsOfflineV2ReadinessMalformedAbi7Aliases[\s\S]*?allowsOfflineV2ReadinessMalformedAbi7Aliases/u,
    "mobile offline readiness negative control must mutate client and parser coverage",
  );
  assert.match(
    mobileOfflineReadinessBranch,
    /Kotlin Offline readiness client tests[\s\S]*?Kotlin Offline V2 readiness client tests[\s\S]*?Android Java Offline Torii readiness client tests[\s\S]*?Android Java Offline readiness parser tests/u,
    "mobile offline readiness negative control must require all mobile readiness labels",
  );
  assert.match(
    mobileOfflineReadinessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: mobile offline readiness coverage drift was not detected"\)/u,
    "mobile offline readiness negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobileOfflineReadinessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile offline readiness negative control must not unconditionally pass after run_checks",
  );
  const kotlinOfflineCashSettlementBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-kotlin-offline-cash-settlement-coverage":'),
    guard.indexOf('if mode == "--negative-control-android-offline-transfer-persistence-coverage":'),
  );
  assert.match(
    kotlinOfflineCashSettlementBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Kotlin offline cash settlement negative control must validate the mutated text snapshot",
  );
  assert.match(
    kotlinOfflineCashSettlementBranch,
    /redeemRequestCommitmentHexMatchesExpected[\s\S]*?redeemRequestCommitmentHexAllowsDrift[\s\S]*?redeemProofsMatchRustFixtures[\s\S]*?redeemProofsAllowRustFixtureDrift/u,
    "Kotlin offline cash settlement negative control must mutate codec and fixture coverage",
  );
  assert.match(
    kotlinOfflineCashSettlementBranch,
    /Kotlin offline cash codec tests[\s\S]*?Kotlin offline settlement proof fixture parity tests/u,
    "Kotlin offline cash settlement negative control must require both Kotlin coverage labels",
  );
  assert.match(
    kotlinOfflineCashSettlementBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Kotlin offline cash settlement coverage drift was not detected"\)/u,
    "Kotlin offline cash settlement negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    kotlinOfflineCashSettlementBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Kotlin offline cash settlement negative control must not unconditionally pass after run_checks",
  );
  const androidOfflineTransferPersistenceBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-android-offline-transfer-persistence-coverage":'),
    guard.indexOf('if mode == "--negative-control-mobile-transaction-norito-runner-coverage":'),
  );
  assert.match(
    androidOfflineTransferPersistenceBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Android offline transfer persistence negative control must validate the mutated text snapshot",
  );
  assert.match(
    androidOfflineTransferPersistenceBranch,
    /recoversMissingChunk[\s\S]*?allowsMissingChunkLoss[\s\S]*?duplicatePendingRejected[\s\S]*?duplicatePendingAllowed[\s\S]*?queuesFailedSubmissionAndEmitsTelemetry[\s\S]*?dropsFailedSubmissionAndTelemetry/u,
    "Android offline transfer persistence negative control must mutate QR, journal, and pending queue coverage",
  );
  assert.match(
    androidOfflineTransferPersistenceBranch,
    /Android offline QR stream tests[\s\S]*?Android offline journal tests[\s\S]*?Android HTTP transport pending queue replay tests/u,
    "Android offline transfer persistence negative control must require QR, journal, and queue labels",
  );
  assert.match(
    androidOfflineTransferPersistenceBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Android offline transfer persistence coverage drift was not detected"\)/u,
    "Android offline transfer persistence negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    androidOfflineTransferPersistenceBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Android offline transfer persistence negative control must not unconditionally pass after run_checks",
  );
  const mobileTransactionNoritoBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-transaction-norito-runner-coverage":'),
    guard.indexOf('if mode == "--negative-control-kotlin-norito-framing-runner-coverage":'),
  );
  assert.match(
    mobileTransactionNoritoBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile transaction/Norito negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobileTransactionNoritoBranch,
    /codec supports instructions and wire payload variants[\s\S]*?codec supports unframed instruction variants[\s\S]*?signed transaction decoder rejects adversarial envelopes[\s\S]*?signed transaction decoder allows adversarial envelopes[\s\S]*?javaCodecRejectsMalformedSignedTransactions[\s\S]*?javaCodecAllowsMalformedSignedTransactions[\s\S]*?fixtureLoaderRejectsWireInstructionArguments[\s\S]*?fixtureLoaderAllowsWireInstructionArguments[\s\S]*?hashIgnoresExportedKeyBundle[\s\S]*?hashIncludesExportedKeyBundle[\s\S]*?roundTripWithExportedKey[\s\S]*?dropsExportedKey/u,
    "mobile transaction/Norito negative control must mutate codec, fixture, hash, and envelope coverage",
  );
  assert.match(
    mobileTransactionNoritoBranch,
    /Kotlin Norito Java codec parity tests[\s\S]*?Kotlin transaction fixture parity tests[\s\S]*?Android Norito codec adapter tests[\s\S]*?Android transaction payload fixture tests[\s\S]*?Android signed transaction hasher tests[\s\S]*?Android offline signing envelope codec tests/u,
    "mobile transaction/Norito negative control must require all transaction/Norito labels",
  );
  assert.match(
    mobileTransactionNoritoBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: mobile transaction\/Norito coverage drift was not detected"\)/u,
    "mobile transaction/Norito negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobileTransactionNoritoBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile transaction/Norito negative control must not unconditionally pass after run_checks",
  );
  const kotlinNoritoFramingBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-kotlin-norito-framing-runner-coverage":'),
    guard.indexOf('if mode == "--negative-control-mobile-account-address-canonical-coverage":'),
  );
  assert.match(
    kotlinNoritoFramingBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Kotlin Norito framing negative control must validate the mutated text snapshot",
  );
  assert.match(
    kotlinNoritoFramingBranch,
    /decode rejects reserved layout flags[\s\S]*?decode permits reserved layout flags[\s\S]*?decode rejects field bitset without required flags[\s\S]*?decode permits field bitset without required flags[\s\S]*?optional string columnar matches Rust golden and rejects malformed payloads[\s\S]*?optional string columnar permits malformed payloads[\s\S]*?optional u32 columnar matches Rust golden and rejects malformed AoS[\s\S]*?optional u32 columnar permits malformed AoS[\s\S]*?bytes bool columnar and adaptive layouts match Rust goldens[\s\S]*?bytes bool columnar accepts trailing flags/u,
    "Kotlin Norito framing negative control must mutate header and columnar coverage",
  );
  assert.match(
    kotlinNoritoFramingBranch,
    /Kotlin Norito header layout flag tests[\s\S]*?Kotlin Norito columnar golden\/adversarial tests/u,
    "Kotlin Norito framing negative control must require both Norito labels",
  );
  assert.match(
    kotlinNoritoFramingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Kotlin Norito framing coverage drift was not detected"\)/u,
    "Kotlin Norito framing negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    kotlinNoritoFramingBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Kotlin Norito framing negative control must not unconditionally pass after run_checks",
  );
  const mobileAccountAddressBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-account-address-canonical-coverage":'),
    guard.indexOf('if mode == "--negative-control-mobile-connect-runner-coverage":'),
  );
  assert.match(
    mobileAccountAddressBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile account address negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobileAccountAddressBranch,
    /mixedI105LiteralRoundTripsToOriginalCanonicalPayload[\s\S]*?mixedI105LiteralAllowsCanonicalDrift[\s\S]*?rejectsNonCanonicalFullwidthKanaPayload[\s\S]*?allowsNonCanonicalFullwidthKanaPayload[\s\S]*?fromAccountRejectsBlankOrPaddedCurveAlgorithmAliases[\s\S]*?fromAccountAllowsBlankOrPaddedCurveAlgorithmAliases[\s\S]*?fromAccount produces same I105 for known key[\s\S]*?fromAccount permits I105 drift for known key[\s\S]*?complianceFixtureSuite[\s\S]*?skipsComplianceFixtureSuite[\s\S]*?i105RejectsFullwidthSentinel[\s\S]*?i105AllowsFullwidthSentinel[\s\S]*?curveAlgorithmAliasesRejectBlankAndPaddedLabels[\s\S]*?curveAlgorithmAliasesAllowBlankAndPaddedLabels/u,
    "mobile account address negative control must mutate Kotlin and Android account canonical coverage",
  );
  assert.match(
    mobileAccountAddressBranch,
    /Kotlin AccountAddress canonical tests[\s\S]*?Kotlin I105 canonical SPKI tests[\s\S]*?Android AccountAddress compliance tests/u,
    "mobile account address negative control must require all account address labels",
  );
  assert.match(
    mobileAccountAddressBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\s*\(\s*"negative control failed: mobile account address canonical coverage drift was not detected"\s*\)/u,
    "mobile account address negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobileAccountAddressBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile account address negative control must not unconditionally pass after run_checks",
  );
  const mobileConnectBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-connect-runner-coverage":'),
    guard.indexOf('if mode == "--negative-control-mobile-transport-inspector-attestation-coverage":'),
  );
  assert.match(
    mobileConnectBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile Connect negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobileConnectBranch,
    /deriveDirectionKeysRejectsLowOrderPeerPublicKey[\s\S]*?deriveDirectionKeysAllowsLowOrderPeerPublicKey[\s\S]*?buildApprovePreimageRejectsDomainQualifiedAccountAlias[\s\S]*?buildApprovePreimageAllowsDomainQualifiedAccountAlias[\s\S]*?signResultOkAcceptsExactEd25519Algorithm[\s\S]*?signResultOkAcceptsLooseEd25519Algorithm[\s\S]*?negativeSequenceRejectedAcrossConnectSurfaces[\s\S]*?negativeSequenceAcceptedAcrossConnectSurfaces[\s\S]*?relayAuthHashMatchesSharedFixture[\s\S]*?relayAuthHashDriftsFromSharedFixture[\s\S]*?decodeLiveSignRequestRawFixture[\s\S]*?skipLiveSignRequestRawFixture[\s\S]*?pruneExpiredRecords[\s\S]*?retainExpiredRecords[\s\S]*?testQueueOverflow[\s\S]*?queueOverflowCheckSkipped/u,
    "mobile Connect negative control must mutate Kotlin and Android Connect coverage",
  );
  assert.match(
    mobileConnectBranch,
    /Kotlin Connect crypto tests[\s\S]*?Kotlin Connect envelope codec tests[\s\S]*?Kotlin Connect sequence tests[\s\S]*?Kotlin Connect wallet request tests[\s\S]*?Android Connect envelope\/sequence tests[\s\S]*?Android Connect queue journal tests[\s\S]*?Android Connect retry policy tests[\s\S]*?Android Connect error classifier tests[\s\S]*?Android Connect wallet request tests/u,
    "mobile Connect negative control must require all Connect labels",
  );
  assert.match(
    mobileConnectBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: mobile Connect runner coverage drift was not detected"\)/u,
    "mobile Connect negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobileConnectBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile Connect negative control must not unconditionally pass after run_checks",
  );
  const mobileTransportInspectorAttestationBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-transport-inspector-attestation-coverage":'),
    guard.indexOf('if mode == "--negative-control-mobile-sccp-runner-coverage":'),
  );
  assert.match(
    mobileTransportInspectorAttestationBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile transport/inspector/attestation negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobileTransportInspectorAttestationBranch,
    /executeRunsOnSuppliedAsyncExecutor[\s\S]*?executeMayUseCommonPool[\s\S]*?executeReturns404WithEmptyBodyWhenServerSendsNoContent[\s\S]*?executeReturns404WithoutEmptyBodyAssertion[\s\S]*?inspectModernEntries[\s\S]*?skipModernPendingQueueEntries[\s\S]*?strongBoxAttestationPasses[\s\S]*?skipStrongBoxAttestationPasses[\s\S]*?challengeMismatchFails[\s\S]*?challengeMismatchPasses[\s\S]*?builderRequiresTrustAnchor[\s\S]*?builderAllowsMissingTrustAnchor/u,
    "mobile transport/inspector/attestation negative control must mutate transport, inspector, and attestation coverage",
  );
  assert.match(
    mobileTransportInspectorAttestationBranch,
    /Kotlin URLConnection transport executor tests[\s\S]*?Android URLConnection transport executor tests[\s\S]*?Android pending queue inspector tests[\s\S]*?Android attestation verifier tests/u,
    "mobile transport/inspector/attestation negative control must require all labels",
  );
  assert.match(
    mobileTransportInspectorAttestationBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\s*\(\s*"negative control failed: mobile transport\/inspector\/attestation coverage drift was not detected"\s*\)/u,
    "mobile transport/inspector/attestation negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobileTransportInspectorAttestationBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile transport/inspector/attestation negative control must not unconditionally pass after run_checks",
  );
  const mobileSccpBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-mobile-sccp-runner-coverage":'),
    guard.indexOf('if mode == "--negative-control-mobile-torii-rpc-subscription-websocket-runner-coverage":'),
  );
  assert.match(
    mobileSccpBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile SCCP negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobileSccpBranch,
    /proofRequestBindsPublicSignalsAndRelayContext[\s\S]*?proofRequestSkipsRelayContextBinding[\s\S]*?derivesTronRouteCanaryEvidenceHash[\s\S]*?tronRouteCanaryEvidenceHashDrifts[\s\S]*?derivesTonRouteCanaryEvidenceHash[\s\S]*?tonRouteCanaryEvidenceHashDrifts[\s\S]*?derivesSolanaRouteCanaryEvidenceHash[\s\S]*?solanaRouteCanaryEvidenceHashDrifts[\s\S]*?derivesSourceAdapterVerifierVkHashesForUiTooling[\s\S]*?sourceAdapterVerifierVkHashesDriftForUiTooling/u,
    "mobile SCCP negative control must mutate EVM, TRON, TON, Solana, and source proof hash coverage",
  );
  assert.match(
    mobileSccpBranch,
    /Kotlin SCCP EVM prover tests[\s\S]*?Android SCCP EVM prover tests[\s\S]*?Kotlin SCCP TRON prover tests[\s\S]*?Android SCCP TRON prover tests[\s\S]*?Kotlin SCCP TON prover tests[\s\S]*?Android SCCP TON prover tests[\s\S]*?Kotlin SCCP Solana prover tests[\s\S]*?Android SCCP Solana prover tests[\s\S]*?Kotlin SCCP source proof hash tests[\s\S]*?Android SCCP source proof hash tests/u,
    "mobile SCCP negative control must require all SCCP labels",
  );
  assert.match(
    mobileSccpBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: mobile SCCP runner coverage drift was not detected"\)/u,
    "mobile SCCP negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobileSccpBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile SCCP negative control must not unconditionally pass after run_checks",
  );
  const mobileToriiRpcSubscriptionWebSocketBranch = guard.slice(
    guard.indexOf(
      'if mode == "--negative-control-mobile-torii-rpc-subscription-websocket-runner-coverage":',
    ),
    guard.indexOf('if mode == "--negative-control-jvm-sdk-android-harness-script":'),
  );
  assert.match(
    mobileToriiRpcSubscriptionWebSocketBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "mobile Torii RPC/subscription/WebSocket negative control must validate the mutated text snapshot",
  );
  assert.match(
    mobileToriiRpcSubscriptionWebSocketBranch,
    /noritoRpcRejectsInsecureAuthorizationHeader[\s\S]*?noritoRpcAllowsInsecureAuthorizationHeader[\s\S]*?defaultHeadersAndPayloadAreApplied[\s\S]*?defaultHeadersAndPayloadAreSkipped[\s\S]*?createPlanRejectsInsecureTransportForPrivateKeyBody[\s\S]*?createPlanAllowsInsecureTransportForPrivateKeyBody[\s\S]*?staleEventsAreIgnored[\s\S]*?staleEventsAreDelivered[\s\S]*?connectRejectsInsecureCredentialedWebSocket[\s\S]*?connectAllowsInsecureCredentialedWebSocket[\s\S]*?staleMessagesAreIgnored[\s\S]*?staleMessagesAreDelivered[\s\S]*?recordsSubmitRequests[\s\S]*?dropsSubmitRequests/u,
    "mobile Torii RPC/subscription/WebSocket negative control must mutate transport, RPC, subscription, SSE, WebSocket, and mock-server coverage",
  );
  assert.match(
    mobileToriiRpcSubscriptionWebSocketBranch,
    /Kotlin Torii transport security tests[\s\S]*?Android Norito RPC client tests[\s\S]*?Android ClientConfig Norito RPC tests[\s\S]*?Android Subscription Torii client tests[\s\S]*?Android Torii SSE subscription tests[\s\S]*?Android Torii WebSocket client tests[\s\S]*?Android Torii WebSocket subscription tests[\s\S]*?Android Torii mock server tests/u,
    "mobile Torii RPC/subscription/WebSocket negative control must require all Torii RPC/subscription labels",
  );
  assert.match(
    mobileToriiRpcSubscriptionWebSocketBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\s*\(\s*"negative control failed: mobile Torii RPC\/subscription\/WebSocket coverage drift was not detected"\s*\)/u,
    "mobile Torii RPC/subscription/WebSocket negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    mobileToriiRpcSubscriptionWebSocketBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "mobile Torii RPC/subscription/WebSocket negative control must not unconditionally pass after run_checks",
  );
  const jsToriiRunnerBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-torii-runner-coverage":'),
    guard.indexOf('if mode == "--negative-control-js-connect-runner-coverage":'),
  );
  assert.match(
    jsToriiRunnerBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "JavaScript Torii runner negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsToriiRunnerBranch,
    /ToriiClient attaches canonical signing headers for app endpoints[\s\S]*?ToriiClient skips canonical signing headers for app endpoints[\s\S]*?subscription action endpoints send normalized payloads[\s\S]*?subscription action endpoints skip normalized payloads[\s\S]*?buildConnectWebSocketUrl rejects token query parameters[\s\S]*?buildConnectWebSocketUrl allows token query parameters[\s\S]*?resolveAliasByIndex enforces non-negative indices before issuing requests[\s\S]*?resolveAliasByIndex allows negative indices before issuing requests/u,
    "JavaScript Torii runner negative control must mutate canonical auth, subscription, WebSocket, and ISO alias tests",
  );
  assert.match(
    jsToriiRunnerBranch,
    /JavaScript Torii canonical auth tests[\s\S]*?JavaScript Torii subscription tests[\s\S]*?JavaScript Connect WebSocket tests[\s\S]*?JavaScript ISO alias tests/u,
    "JavaScript Torii runner negative control must require all Torii labels",
  );
  assert.match(
    jsToriiRunnerBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JavaScript Torii runner coverage drift was not detected"\)/u,
    "JavaScript Torii runner negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsToriiRunnerBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JavaScript Torii runner negative control must not unconditionally pass after run_checks",
  );
  const jsConnectRunnerBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-connect-runner-coverage":'),
    guard.indexOf('if mode == "--negative-control-js-sdk-workflow-inventory":'),
  );
  assert.match(
    jsConnectRunnerBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "JavaScript Connect runner negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsConnectRunnerBranch,
    /generateConnectSid derives deterministic sid[\s\S]*?generateConnectSid derives random sid[\s\S]*?connect queue overflow maps to queueOverflow category[\s\S]*?connect queue overflow maps to success category[\s\S]*?connect retry deterministic series for zero seed[\s\S]*?connect retry random series for zero seed[\s\S]*?memory journal enforces limits[\s\S]*?memory journal ignores limits[\s\S]*?connect queue diagnostics snapshot \+ evidence[\s\S]*?connect queue diagnostics drops evidence[\s\S]*?createConnectAppSession handles approval and sign success[\s\S]*?createConnectAppSession skips approval and sign success[\s\S]*?bootstrapConnectPreviewSession registers by default[\s\S]*?bootstrapConnectPreviewSession skips default registration[\s\S]*?ConnectJournalRecord header matches Norito v1 defaults[\s\S]*?ConnectJournalRecord header ignores Norito v1 defaults/u,
    "JavaScript Connect runner negative control must mutate session, error, retry, journal, diagnostics, browser, preview, and record coverage",
  );
  assert.match(
    jsConnectRunnerBranch,
    /JavaScript Connect session tests[\s\S]*?JavaScript Connect error tests[\s\S]*?JavaScript Connect retry policy tests[\s\S]*?JavaScript Connect queue journal tests[\s\S]*?JavaScript Connect queue diagnostics tests[\s\S]*?JavaScript Connect browser tests[\s\S]*?JavaScript Connect preview flow tests[\s\S]*?JavaScript Connect journal record tests/u,
    "JavaScript Connect runner negative control must require all Connect labels",
  );
  assert.match(
    jsConnectRunnerBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JavaScript Connect runner coverage drift was not detected"\)/u,
    "JavaScript Connect runner negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsConnectRunnerBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JavaScript Connect runner negative control must not unconditionally pass after run_checks",
  );
  const jsNativeBuildWorkflowBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-sdk-native-build-workflow":'),
    guard.indexOf('if mode == "--negative-control-js-sdk-test-workflow":'),
  );
  assert.match(
    jsNativeBuildWorkflowBranch,
    /JS_SDK_NATIVE_BUILD_COMMAND[\s\S]*?npm run build:dist --prefix javascript\/iroha_js[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(texts\)/u,
    "JavaScript SDK native-build workflow negative control must mutate and validate the workflow build command",
  );
  assert.match(
    jsNativeBuildWorkflowBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JavaScript SDK native build workflow drift was not detected"\)/u,
    "JavaScript SDK native-build workflow negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsNativeBuildWorkflowBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JavaScript SDK native-build workflow negative control must not unconditionally pass after run_checks",
  );
  const jsNativeBuildOrderBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-sdk-native-build-order-workflow":'),
    guard.indexOf('if mode == "--negative-control-js-sdk-needs-workflow":'),
  );
  assert.match(
    jsNativeBuildOrderBranch,
    /Build JavaScript SDK native host[\s\S]*?JS_SDK_NATIVE_BUILD_COMMAND[\s\S]*?JS_SDK_TEST_COMMAND[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(texts\)/u,
    "JavaScript SDK native-build ordering negative control must move and validate the native build step",
  );
  assert.match(
    jsNativeBuildOrderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JavaScript SDK native build ordering drift was not detected"\)/u,
    "JavaScript SDK native-build ordering negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsNativeBuildOrderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JavaScript SDK native-build ordering negative control must not unconditionally pass after run_checks",
  );
  const csharpArchiveCopyBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-csharp-archive-copy":'),
    guard.indexOf('if mode == "--negative-control-csharp-recursive-compact-verifier-unavailable":'),
  );
  assert.match(
    csharpArchiveCopyBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[[^\]]+\]\s*=\s*mutated_(?:test|source)[\s\S]*?run_checks\(mutated_texts\)/u,
    "C# archive wrapper copy negative control must validate the mutated text snapshot",
  );
  assert.match(
    csharpArchiveCopyBranch,
    /invalidFieldBitset\[39\] = 0x20[\s\S]*?invalidFieldBitset\[39\] = 0x06/u,
    "C# archive wrapper copy negative control must mutate malformed-header coverage",
  );
  assert.match(
    csharpArchiveCopyBranch,
    /AssertRejectsMalformedEverywhere\(invalidFieldBitset, validArchive\)[\s\S]*?AssertRejectsMalformedEverywhere\(validArchive, validArchive\)/u,
    "C# archive wrapper copy negative control must mutate native input malformed-header coverage",
  );
  assert.match(
    csharpArchiveCopyBranch,
    /AssertRejectsMalformedBridgeOutput\(invalidFieldBitset\)[\s\S]*?AssertRejectsMalformedBridgeOutput\(KagemushaNoritoFrameWithPayload\(0x4b\)\)/u,
    "C# archive wrapper copy negative control must mutate native output malformed-header coverage",
  );
  assert.match(
    csharpArchiveCopyBranch,
    /C# recursive spend input Norito guard tests/u,
    "C# archive wrapper copy negative control must require native input guard test drift detection",
  );
  assert.match(
    csharpArchiveCopyBranch,
    /C# recursive compact verifier tests/u,
    "C# archive wrapper copy negative control must require native output guard test drift detection",
  );
  assert.match(
    csharpArchiveCopyBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: C# archive wrapper copy drift was not detected"\)/u,
    "C# archive wrapper copy negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    csharpArchiveCopyBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "C# archive wrapper copy negative control must not unconditionally pass after run_checks",
  );
  const csharpRecursiveCompactVerifierUnavailableBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-csharp-recursive-compact-verifier-unavailable":'),
    guard.indexOf('if mode == "--negative-control-csharp-sdk-test-workflow":'),
  );
  assert.match(
    csharpRecursiveCompactVerifierUnavailableBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "C# recursive compact verifier unavailable negative control must validate the mutated text snapshot",
  );
  assert.match(
    csharpRecursiveCompactVerifierUnavailableBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: C# recursive compact verifier unavailable drift was not detected"\)/u,
    "C# recursive compact verifier unavailable negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    csharpRecursiveCompactVerifierUnavailableBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "C# recursive compact verifier unavailable negative control must not unconditionally pass after run_checks",
  );
  const swiftRecursiveCompactVerifierBoolBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-recursive-compact-verifier-bool":'),
    guard.indexOf('if mode == "--negative-control-swift-recursive-compact-verifier-availability":'),
  );
  assert.match(
    swiftRecursiveCompactVerifierBoolBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift recursive compact verifier bool negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftRecursiveCompactVerifierBoolBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift recursive compact verifier bool drift was not detected"\)/u,
    "Swift recursive compact verifier bool negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftRecursiveCompactVerifierBoolBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift recursive compact verifier bool negative control must not unconditionally pass after run_checks",
  );
  const swiftRecursiveCompactVerifierAvailabilityBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-recursive-compact-verifier-availability":'),
    guard.indexOf('if mode == "--negative-control-swift-kagemusha-native-output-cap":'),
  );
  assert.match(
    swiftRecursiveCompactVerifierAvailabilityBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift recursive compact verifier availability negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftRecursiveCompactVerifierAvailabilityBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift recursive compact verifier availability drift was not detected"\)/u,
    "Swift recursive compact verifier availability negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftRecursiveCompactVerifierAvailabilityBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift recursive compact verifier availability negative control must not unconditionally pass after run_checks",
  );
  const swiftKagemushaNativeOutputCapBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-kagemusha-native-output-cap":'),
    guard.indexOf('if mode == "--negative-control-swift-native-output-headers":'),
  );
  assert.match(
    swiftKagemushaNativeOutputCapBranch,
    /length <= CUnsignedLong\(KagemushaRecursiveSpendProver\.nativeArchiveMaxBytes\)[\s\S]*?true \|\| length <= CUnsignedLong\(KagemushaRecursiveSpendProver\.nativeArchiveMaxBytes\)[\s\S]*?testNativeBridgeRejectsOversizedKagemushaOutputBeforeCopying[\s\S]*?testNativeBridgeAllowsOversizedKagemushaOutputBeforeCopying/u,
    "Swift Kagemusha native output cap negative control must weaken the bridge cap and cap-plus-one test",
  );
  assert.match(
    swiftKagemushaNativeOutputCapBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[bridge\]\s*=\s*mutated_bridge[\s\S]*?mutated_texts\[test_path\]\s*=\s*mutated_test[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift Kagemusha native output cap negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftKagemushaNativeOutputCapBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift Kagemusha native output cap drift was not detected"\)/u,
    "Swift Kagemusha native output cap negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftKagemushaNativeOutputCapBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift Kagemusha native output cap negative control must not unconditionally pass after run_checks",
  );
  const swiftNativeOutputHeaderBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-native-output-headers":'),
    guard.indexOf('if mode == "--negative-control-swift-native-input-headers":'),
  );
  assert.match(
    swiftNativeOutputHeaderBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift native output header negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftNativeOutputHeaderBranch,
    /invalidFieldBitset\[39\] = NoritoHeader\.fieldBitset[\s\S]*?invalidFieldBitset\[39\] = NoritoHeader\.packedStruct \| NoritoHeader\.compactLen/u,
    "Swift native output header negative control must mutate malformed-header coverage",
  );
  assert.match(
    swiftNativeOutputHeaderBranch,
    /Swift recursive spend native output header guard tests[\s\S]*?Swift compact-token native output header guard tests[\s\S]*?Swift recursive aggregation native output header guard tests[\s\S]*?Swift recursive compact native output header guard tests/u,
    "Swift native output header negative control must require every Swift guard label",
  );
  assert.match(
    swiftNativeOutputHeaderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift native output header drift was not detected"\)/u,
    "Swift native output header negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftNativeOutputHeaderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift native output header negative control must not unconditionally pass after run_checks",
  );
  const swiftNativeInputHeaderBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-native-input-headers":'),
    guard.indexOf('if mode == "--negative-control-swift-kagemusha-instruction-transaction-builder":'),
  );
  assert.match(
    swiftNativeInputHeaderBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift native input header negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftNativeInputHeaderBranch,
    /invalidFieldBitset\[39\] = NoritoHeader\.fieldBitset[\s\S]*?invalidFieldBitset\[39\] = NoritoHeader\.packedStruct \| NoritoHeader\.compactLen/u,
    "Swift native input header negative control must mutate malformed-header coverage",
  );
  assert.match(
    swiftNativeInputHeaderBranch,
    /Swift recursive spend input header guard tests[\s\S]*?Swift compact-token input header guard tests[\s\S]*?Swift recursive aggregation input header guard tests[\s\S]*?Swift recursive compact input header guard tests/u,
    "Swift native input header negative control must require every Swift input guard label",
  );
  assert.match(
    swiftNativeInputHeaderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift native input header drift was not detected"\)/u,
    "Swift native input header negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftNativeInputHeaderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift native input header negative control must not unconditionally pass after run_checks",
  );
  const swiftInstructionTransactionBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-kagemusha-instruction-transaction-builder":'),
    guard.indexOf('if mode == "--negative-control-swift-identifier-receipt-account-id-decode-test":'),
  );
  assert.match(
    swiftInstructionTransactionBranch,
    /func buildKagemushaRecursiveRedeem\([\s\S]*?func buildKagemushaRecursiveRedeemUnchecked\([\s\S]*?testBuildKagemushaRecursiveRedeemTransactionDerivesInstructionBeforeSigning[\s\S]*?testBuildKagemushaRecursiveRedeemTransactionSkipsNativeDerivationBeforeSigning/u,
    "Swift instruction transaction builder negative control must mutate the recursive redeem builder and derivation test",
  );
  assert.match(
    swiftInstructionTransactionBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[source_target\]\s*=\s*mutated_source[\s\S]*?mutated_texts\[test_target\]\s*=\s*mutated_test[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift instruction transaction builder negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftInstructionTransactionBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: Swift Kagemusha instruction transaction builder drift was not detected"\s*\)/u,
    "Swift instruction transaction builder negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftInstructionTransactionBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift instruction transaction builder negative control must not unconditionally pass after run_checks",
  );
  const swiftIdentifierReceiptAccountIdBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-identifier-receipt-account-id-decode-test":'),
    guard.indexOf('if mode == "--negative-control-swift-nfc-receive-success-preservation":'),
  );
  assert.match(
    swiftIdentifierReceiptAccountIdBranch,
    /testIdentifierReceiptDecodeRejectsPaddedAccountIdBeforeSignatureVerification[\s\S]*?testIdentifierReceiptDecodeAllowsPaddedAccountIdBeforeSignatureVerification/u,
    "Swift identifier receipt account-id negative control must mutate the padded account-id decode regression",
  );
  assert.match(
    swiftIdentifierReceiptAccountIdBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift identifier receipt account-id negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftIdentifierReceiptAccountIdBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: Swift identifier receipt account-id decode coverage drift was not detected"\s*\)/u,
    "Swift identifier receipt account-id negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftIdentifierReceiptAccountIdBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift identifier receipt account-id negative control must not unconditionally pass after run_checks",
  );
  const swiftNfcReceiveSuccessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-nfc-receive-success-preservation":'),
    guard.indexOf('if mode == "--negative-control-swift-nfc-receipt-ack-single-success":'),
  );
  assert.match(
    swiftNfcReceiveSuccessBranch,
    /hasSuccessState: didPublishReceiveSuccess \|\| didComplete[\s\S]*?hasSuccessState: didComplete/u,
    "Swift NFC receive success preservation negative control must remove the published-success state",
  );
  assert.match(
    swiftNfcReceiveSuccessBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift NFC receive success preservation negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftNfcReceiveSuccessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift NFC receive success preservation drift was not detected"\)/u,
    "Swift NFC receive success preservation negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftNfcReceiveSuccessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift NFC receive success preservation negative control must not unconditionally pass after run_checks",
  );
  const swiftNfcAckSingleSuccessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-nfc-receipt-ack-single-success":'),
    guard.indexOf('if mode == "--negative-control-swift-nfc-receipt-ack-read-single-success":'),
  );
  assert.match(
    swiftNfcAckSingleSuccessBranch,
    /let shouldNotifyReceiptAckReady = markReceiptAckReady\(\)[\s\S]*?let shouldNotifyReceiptAckReady = true/u,
    "Swift NFC receipt ACK single-success negative control must bypass the ACK-ready gate",
  );
  assert.match(
    swiftNfcAckSingleSuccessBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift NFC receipt ACK single-success negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftNfcAckSingleSuccessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift NFC receipt ACK single-success drift was not detected"\)/u,
    "Swift NFC receipt ACK single-success negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftNfcAckSingleSuccessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift NFC receipt ACK single-success negative control must not unconditionally pass after run_checks",
  );
  const swiftNfcAckReadSingleSuccessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-nfc-receipt-ack-read-single-success":'),
    guard.indexOf('if mode == "--negative-control-swift-nfc-emulation-progress-after-success":'),
  );
  assert.match(
    swiftNfcAckReadSingleSuccessBranch,
    /shouldPublishReceiveSuccessOnAckRead\([\s\S]*?hasAcceptedPayment: didAcceptPayment[\s\S]*?hasAcceptedPayment: false/u,
    "Swift NFC receipt ACK-read single-success negative control must bypass the ACK-read accepted-payment gate",
  );
  assert.match(
    swiftNfcAckReadSingleSuccessBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift NFC receipt ACK-read single-success negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftNfcAckReadSingleSuccessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift NFC receipt ACK-read single-success drift was not detected"\)/u,
    "Swift NFC receipt ACK-read single-success negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftNfcAckReadSingleSuccessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift NFC receipt ACK-read single-success negative control must not unconditionally pass after run_checks",
  );
  const swiftNfcEmulationProgressBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-nfc-emulation-progress-after-success":'),
    guard.indexOf('if mode == "--negative-control-swift-nfc-send-terminal-success-policy":'),
  );
  assert.match(
    swiftNfcEmulationProgressBranch,
    /hasAcceptedPayment: didAcceptPayment[\s\S]*?hasAcceptedPayment: false/u,
    "Swift NFC emulation progress-after-success negative control must ignore accepted-payment state",
  );
  assert.match(
    swiftNfcEmulationProgressBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift NFC emulation progress-after-success negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftNfcEmulationProgressBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift NFC emulation progress-after-success drift was not detected"\)/u,
    "Swift NFC emulation progress-after-success negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftNfcEmulationProgressBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift NFC emulation progress-after-success negative control must not unconditionally pass after run_checks",
  );
  const swiftNfcSendTerminalSuccessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-nfc-send-terminal-success-policy":'),
    guard.indexOf('if mode == "--negative-control-swift-sdk-version-script":'),
  );
  assert.match(
    swiftNfcSendTerminalSuccessBranch,
    /hasTerminalSuccess: hasTerminalSendSuccess[\s\S]*?hasTerminalSuccess: false/u,
    "Swift NFC send terminal-success negative control must ignore terminal send-success state",
  );
  assert.match(
    swiftNfcSendTerminalSuccessBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "Swift NFC send terminal-success negative control must validate the mutated text snapshot",
  );
  assert.match(
    swiftNfcSendTerminalSuccessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift NFC send terminal-success policy drift was not detected"\)/u,
    "Swift NFC send terminal-success negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    swiftNfcSendTerminalSuccessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Swift NFC send terminal-success negative control must not unconditionally pass after run_checks",
  );
  const jsInstructionTransactionBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-kagemusha-instruction-transaction-builder":'),
    guard.indexOf('if mode == "--negative-control-js-python-native-output-headers":'),
  );
  assert.match(
    jsInstructionTransactionBranch,
    /export function buildKagemushaRecursiveRedeemTransaction[\s\S]*?export function buildKagemushaRecursiveRedeemUncheckedTransaction[\s\S]*?buildKagemushaRecursiveRedeemTransaction derives instruction before signing[\s\S]*?buildKagemushaRecursiveRedeemTransaction skips instruction derivation before signing/u,
    "JS instruction transaction builder negative control must mutate the builder and redeem-before-sign test",
  );
  assert.match(
    jsInstructionTransactionBranch,
    /mutated\s*=\s*dict\(texts\)[\s\S]*?mutated\[source_target\]\s*=\s*mutated_source[\s\S]*?mutated\[test_target\]\s*=\s*mutated_test[\s\S]*?run_checks\(mutated\)/u,
    "JS instruction transaction builder negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsInstructionTransactionBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: JS Kagemusha instruction transaction builder drift was not detected"\s*\)/u,
    "JS instruction transaction builder negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsInstructionTransactionBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS instruction transaction builder negative control must not unconditionally pass after run_checks",
  );
  const jsPythonNativeOutputHeaderBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-python-native-output-headers":'),
    guard.indexOf('if mode == "--negative-control-python-kagemusha-instruction-transaction-builder":'),
  );
  assert.match(
    jsPythonNativeOutputHeaderBranch,
    /mutated\s*=\s*dict\(texts\)[\s\S]*?mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "JS/Python native output header negative control must validate the mutated text snapshot",
  );
  assert.match(
    jsPythonNativeOutputHeaderBranch,
    /invalidFieldBitset\[39\] = 0x20[\s\S]*?invalidFieldBitset\[39\] = 0x06[\s\S]*?invalid_field_bitset\[39\] = 0x20[\s\S]*?invalid_field_bitset\[39\] = 0x06/u,
    "JS/Python native output header negative control must mutate both malformed-header cases",
  );
  assert.match(
    jsPythonNativeOutputHeaderBranch,
    /JavaScript recursive spend native output header guard tests[\s\S]*?Python recursive spend native output header guard tests/u,
    "JS/Python native output header negative control must require both guard labels",
  );
  assert.match(
    jsPythonNativeOutputHeaderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JS\/Python native output header drift was not detected"\)/u,
    "JS/Python native output header negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    jsPythonNativeOutputHeaderBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS/Python native output header negative control must not unconditionally pass after run_checks",
  );
  const pythonAbi7FixtureNativeGuardBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-python-sdk-abi7-fixture-native-guard":'),
    guard.indexOf('if mode == "--negative-control-abi7-sdk-manifest-coverage":'),
  );
  assert.match(
    pythonAbi7FixtureNativeGuardBranch,
    /PYTHON_ABI7_FIXTURE_NATIVE_GUARD_COMMAND[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(texts\)/u,
    "Python ABI-7 fixture native guard negative control must validate the mutated runner text",
  );
  assert.match(
    pythonAbi7FixtureNativeGuardBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Python SDK ABI-7 fixture native guard drift was not detected"\)/u,
    "Python ABI-7 fixture native guard negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    pythonAbi7FixtureNativeGuardBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Python ABI-7 fixture native guard negative control must not unconditionally pass after run_checks",
  );
  const abi7SdkManifestCoverageBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-abi7-sdk-manifest-coverage":'),
    guard.indexOf('if mode == "--negative-control-python-connect-runner-coverage":'),
  );
  const abi7SdkManifestCoverageInventory = guard.slice(
    guard.indexOf("ABI7_SDK_MANIFEST_COVERAGE = {"),
    guard.indexOf("JVM_SDK_TEST_COMMAND ="),
  );
  assertContainsAll(
    abi7SdkManifestCoverageInventory,
    [
      "python/iroha_python/tests/kagemusha_test.py",
      "_shared_recursive_spend_abi7_manifest",
      "test_recursive_kagemusha_shared_abi7_fixture_manifest_matches_archives_and_generator",
      "assert set(manifest) ==",
      "assert set(archive) ==",
      "len(archive_entries) == len(expected_operations)",
      "hashlib.sha256(archive_bytes).hexdigest()",
      "javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
      "sharedRecursiveSpendAbi7Manifest",
      "Kagemusha recursive spend shared ABI-7 fixture manifest matches archive fixture",
      "Object.keys(manifest).sort()",
      "Object.keys(archive).sort()",
      "archiveFixture.archives.length, expectedOperations.size",
      "createHash(\"sha256\").update(archiveBytes).digest(\"hex\")",
      "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift",
      "testSharedRecursiveSpendAbi7ManifestMatchesArchiveFixture",
      "Set(manifest.keys)",
      "Set(archive.keys)",
      "archives.count, expectedOperations.count",
      "SHA256.hash(data: archiveBytes)",
      "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendRequestCodecsTest.kt",
      "ABI 7 fixture manifest matches archive fixture",
      "manifest.keys",
      "archive.keys",
      "expectedOperations.size, archives.size",
      "sha256Hex(archiveBytes)",
      "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
      "sharedRecursiveSpendAbi7FixtureManifestMatchesArchiveFixture",
      "assertKeySet(",
      "byte_len\\\", \\\"sha256_hex\\\", \\\"bytes_base64",
      "archives.size() == expectedNames.size()",
      "archiveBytes.length",
    ],
    "ABI-7 SDK fixture manifest coverage inventory must cover every non-C# SDK test surface",
  );
  assert.match(
    abi7SdkManifestCoverageBranch,
    /for\s+target,\s+needles\s+in\s+ABI7_SDK_MANIFEST_COVERAGE\.items\(\):[\s\S]*?for\s+needle\s+in\s+needles:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(texts\)[\s\S]*?text_overrides\.pop\(target,\s*None\)/u,
    "ABI-7 SDK fixture manifest coverage negative control must validate each mutated SDK snapshot",
  );
  assert.match(
    abi7SdkManifestCoverageBranch,
    /ABI-7 SDK fixture manifest coverage[\s\S]*?target not in message[\s\S]*?needle not in message[\s\S]*?raise\s+SystemExit\(0\)/u,
    "ABI-7 SDK fixture manifest coverage negative control must only pass after detecting all injected drifts",
  );
  const jsRunner = source("ci/check_kagemusha_recursive_spend_js_sdk.sh");
  const pythonRunner = source("ci/check_kagemusha_recursive_spend_python_sdk.sh");
  const swiftRunner = source("ci/check_kagemusha_recursive_spend_swift_sdk.sh");
  const csharpRunner = source("ci/check_kagemusha_recursive_spend_csharp_sdk.sh");
  const jvmRunner = source("ci/check_kagemusha_recursive_spend_jvm_sdk.sh");
  assert.match(
    pythonRunner,
    /cargo test -p iroha_python_rs kagemusha_recursive_spend_abi7_archive_fixture_matches_python_native_bridge -- --nocapture[\s\S]*"\$\{VENV_DIR\}\/bin\/python" -m maturin develop --release/u,
    "Kagemusha Python SDK runner must validate ABI-7 archive fixtures before building the wheel",
  );
  assert.match(
    jvmRunner,
    /JAVA_HOME_OVERRIDE="\$\{KAGEMUSHA_RECURSIVE_SPEND_JVM_JAVA_HOME:-\}"/,
    "Kagemusha JVM SDK runner must keep the documented Java home override variable",
  );
  assert.match(
    jvmRunner,
    /JAVA_HOME must point to a JDK 21 home for Kagemusha recursive spend JVM SDK tests\./,
    "Kagemusha JVM SDK runner must reject inherited non-JDK-21 JAVA_HOME values",
  );
  assert.match(
    jvmRunner,
    /--tests org\.hyperledger\.iroha\.sdk\.privacy\.PrivacyNativeBridgeTest[\s\S]*ANDROID_HARNESS_MAINS=[^\n]*org\.hyperledger\.iroha\.android\.privacy\.PrivacyNativeBridgeTest/,
    "Kagemusha JVM SDK runner must exercise Kotlin and Android privacy native bridge tests",
  );
  assert.match(
    jvmRunner,
    /--tests org\.hyperledger\.iroha\.sdk\.offline\.KagemushaRecursiveSpendRequestCodecsTest[\s\S]*ANDROID_HARNESS_MAINS=[^\n]*org\.hyperledger\.iroha\.android\.offline\.KagemushaRecursiveSpendProverTest/,
    "Kagemusha JVM SDK runner must exercise typed recursive spend request-codec tests",
  );
  assert.match(
    jvmRunner,
    /--tests org\.hyperledger\.iroha\.sdk\.address\.AccountIdLiteralTest[\s\S]*--tests org\.hyperledger\.iroha\.sdk\.offline\.OfflineCashLifecycleTest[\s\S]*ANDROID_HARNESS_MAINS=[^\n]*org\.hyperledger\.iroha\.android\.offline\.OfflineCashLifecycleTest[^\n]*org\.hyperledger\.iroha\.android\.address\.AccountIdLiteralTests/,
    "Kagemusha JVM SDK runner must exercise Kotlin and Android account literal and offline cash issuer-key exactness tests",
  );
  assert.match(
    jvmRunner,
    /--tests org\.hyperledger\.iroha\.sdk\.client\.CanonicalRequestSignerTest[\s\S]*ANDROID_HARNESS_MAINS=[^\n]*org\.hyperledger\.iroha\.android\.client\.CanonicalRequestSignerTests/,
    "Kagemusha JVM SDK runner must exercise Kotlin and Android canonical request auth exactness tests",
  );
  assert.match(
    jvmRunner,
    /--tests org\.hyperledger\.iroha\.sdk\.core\.model\.instructions\.VerifyingKeyInstructionBuildersTest[\s\S]*--tests org\.hyperledger\.iroha\.sdk\.core\.model\.zk\.VerifyingKeyBackendTagTest[\s\S]*--tests org\.hyperledger\.iroha\.sdk\.core\.model\.zk\.VerifyingKeyRecordDescriptionTest[\s\S]*--tests org\.hyperledger\.iroha\.sdk\.core\.model\.zk\.VerifyingKeyStatusTest[\s\S]*--tests org\.hyperledger\.iroha\.sdk\.crypto\.SigningAlgorithmTest[\s\S]*ANDROID_HARNESS_MAINS=[^\n]*org\.hyperledger\.iroha\.android\.model\.instructions\.VerifyingKeyInstructionUtilsTests[\s\S]*--tests org\.hyperledger\.iroha\.android\.crypto\.SigningAlgorithmTests/,
    "Kagemusha JVM SDK runner must exercise Kotlin and Android signing/verifier-key exactness tests",
  );
  assert.match(
    jvmRunner,
    /--tests org\.hyperledger\.iroha\.sdk\.nexus\.NexusAppClientTest[\s\S]*--tests org\.hyperledger\.iroha\.android\.nexus\.NexusAppClientTest/,
    "Kagemusha JVM SDK runner must exercise Kotlin and Android Nexus wallet signature-algorithm exactness tests",
  );
  assert.match(
    jvmRunner,
    /--tests org\.hyperledger\.iroha\.sdk\.client\.stream\.ToriiEventStreamClientTest[\s\S]*ANDROID_HARNESS_MAINS=[^\n]*org\.hyperledger\.iroha\.android\.client\.stream\.ToriiEventStreamClientTests/,
    "Kagemusha JVM SDK runner must exercise Kotlin and Android Torii event-stream verifier filter exactness tests",
  );
  assert.match(
    jvmRunner,
    /--tests org\.hyperledger\.iroha\.sdk\.client\.TransportSecurityClientTest[\s\S]*ANDROID_HARNESS_MAINS=[^\n]*org\.hyperledger\.iroha\.android\.client\.NoritoRpcClientTests[^\n]*org\.hyperledger\.iroha\.android\.client\.SubscriptionToriiClientTests[^\n]*org\.hyperledger\.iroha\.android\.client\.websocket\.ToriiWebSocketSubscriptionTests[^\n]*org\.hyperledger\.iroha\.android\.client\.mock\.ToriiMockServerTests/,
    "Kagemusha JVM SDK runner must exercise Kotlin transport-security and Android Torii RPC/subscription/WebSocket harness tests",
  );
  assert.match(
    jvmRunner,
    /--tests org\.hyperledger\.iroha\.sdk\.core\.model\.instructions\.ClaimIdentifierWirePayloadEncoderParityTest[\s\S]*ANDROID_HARNESS_MAINS=[^\n]*org\.hyperledger\.iroha\.android\.model\.instructions\.ClaimIdentifierWirePayloadEncoderTests[^\n]*org\.hyperledger\.iroha\.android\.client\.IdentifierReceiptCanonicalEncoderTests/,
    "Kagemusha JVM SDK runner must exercise Kotlin and Android identifier receipt exactness tests",
  );
  assert.match(
    swiftRunner,
    /SWIFTC_BIN="\$\{KAGEMUSHA_RECURSIVE_SPEND_SWIFTC_BIN:-swiftc\}"/,
    "Kagemusha Swift SDK runner must keep the documented swiftc override variable",
  );
  assert.match(
    swiftRunner,
    /IrohaSwift\/Sources\/IrohaSwift\/PrivacyNativeBridge\.swift[\s\S]*IrohaSwift\/Tests\/IrohaSwiftTests\/PrivacyNativeBridgeTests\.swift/,
    "Kagemusha Swift SDK runner must parse the privacy native bridge source and tests",
  );
  for (const swiftOfflinePath of [
    "IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift",
    "IrohaSwift/Sources/IrohaSwift/Crypto.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectAsyncSequence.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectClient.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectCodec.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectCrypto.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectEnvelope.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectEnvelopeCodec.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectError.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectEvents.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectFlowControl.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectFrames.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectKeyStore.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectQueueDiagnostics.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectReplayRecorder.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectRetryPolicy.swift",
    "IrohaSwift/Sources/IrohaSwift/ConnectSession.swift",
    "IrohaSwift/Sources/IrohaSwift/NexusAppClient.swift",
    "IrohaSwift/Sources/IrohaSwift/ToriiClient.swift",
    "IrohaSwift/Sources/IrohaSwift/ToriiCanonicalRequest.swift",
    "IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift",
    "IrohaSwift/Sources/IrohaSwift/OfflineIssuerPublicKey.swift",
    "IrohaSwift/Sources/IrohaSwift/OfflineNoteTextTransferContract.swift",
    "IrohaSwift/Sources/IrohaSwift/OfflineTransferDiagnostics.swift",
    "IrohaSwift/Sources/IrohaSwiftMobileTransports/OfflineNfcMobileTransports.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/CanonicalRequestTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectAsyncSequenceTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectClientTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectCryptoTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectEnvelopeCodecTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectEnvelopeTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectEventsTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectFixtureLoader.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectFixtureLoaderTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectFlowControlTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectFramesTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectKeyStoreTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectQueueDiagnosticsTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectQueueJournalTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectReplayRecorderTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectRetryPolicyTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectSessionBalanceTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectSessionEventStreamTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectSessionTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ConnectTestUtilities.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/IrohaSDKSigningAlgorithmTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/NexusAppClientTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/OfflineIssuerPublicKeyTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteTextTransferContractTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/OfflineReceiptChallengeTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/OfflineTransferDiagnosticsTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTransportUITests/OfflineTransferWidgetTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ToriiCanonicalRequestTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift",
  ]) {
    assert.ok(
      swiftRunner.includes(swiftOfflinePath),
      `Kagemusha Swift SDK runner must parse ${swiftOfflinePath}`,
    );
  }
  const swiftConnectBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-connect-parse-surface-script":'),
    guard.indexOf('if mode == "--negative-control-swift-sdk-privacy-parse-script":'),
  );
  assert.match(
    swiftConnectBranch,
    /IrohaSwift\/Sources\/IrohaSwift\/ConnectClient\.swift[\s\S]*?IrohaSwift\/Tests\/IrohaSwiftTests\/ConnectClientTests\.swift/u,
    "Swift Connect parse negative control must mutate Swift Connect source and test coverage",
  );
  assert.match(
    swiftConnectBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Swift Connect parse surface drift was not detected"\)/u,
    "Swift Connect parse negative control must only pass after detecting injected drift",
  );
  assert.match(
    csharpRunner,
    /DOTNET_BIN="\$\{KAGEMUSHA_RECURSIVE_SPEND_DOTNET_BIN:-dotnet\}"/,
    "Kagemusha C# SDK runner must keep the documented dotnet override variable",
  );
  assert.match(
    csharpRunner,
    /BRIDGE_LIBRARY_NAME="libconnect_norito_bridge\.dylib"[\s\S]*BRIDGE_LIBRARY_NAME="connect_norito_bridge\.dll"[\s\S]*BRIDGE_LIBRARY_NAME="libconnect_norito_bridge\.so"[\s\S]*BRIDGE_LIBRARY_PATH="\$\{BRIDGE_LIBRARY_DIR\}\/\$\{BRIDGE_LIBRARY_NAME\}"[\s\S]*connect_norito_bridge native bridge:/,
    "Kagemusha C# SDK runner must verify and print the freshly built native bridge path",
  );
  assert.match(
    csharpRunner,
    /printf 'dotnet --info:\\n'[\s\S]*"\$\{DOTNET_BIN\}" --info[\s\S]*connect_norito_bridge native bridge sha256:/,
    "Kagemusha C# SDK runner must print host and bridge digest evidence",
  );
  assert.match(
    csharpRunner,
    /FullyQualifiedName~KagemushaRecursiveSpendNativeTests[\s\S]*FullyQualifiedName~PrivacyNativeTests[\s\S]*FullyQualifiedName~TransactionBuilderTests[\s\S]*FullyQualifiedName~CanonicalRequestTests[\s\S]*FullyQualifiedName~ToriiClientTests[\s\S]*FullyQualifiedName~SignedQueryBuilderTests[\s\S]*FullyQualifiedName~SignedIterableQueryBuilderTests[\s\S]*FullyQualifiedName~VerifyingKeyBackendTagTests/,
    "Kagemusha C# SDK runner must exercise canonical request, signed query, and verifier backend exactness tests",
  );
  const csharpVerifyingKeyBackendTests = source(
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/VerifyingKeyBackendTagTests.cs",
  );
  for (const requiredCsharpVerifierBackendTest of [
    "ProductionVerifierBackendClassifierMirrorsNativeAllowlist",
    "ProductionVerifierBackendClassifierRejectsUnsafeLabels",
    "CatalogAliasesRejectNonAsciiConfusablesBeforeCompaction",
    "AdversarialPendingAliasSplicesStayUnsupported",
  ]) {
    assert.ok(
      csharpVerifyingKeyBackendTests.includes(requiredCsharpVerifierBackendTest),
      `Kagemusha C# verifier backend tests must keep ${requiredCsharpVerifierBackendTest}`,
    );
  }
  for (const requiredCsharpVerifierBackendFixture of [
    '" halo2/ipa"',
    '"halo2/ipa "',
    '"stark/fri/sha256-goldilocks "',
    '"halo2\\uFF0Fipa"',
    '"halo2/\\u200Bipa"',
    '"h\\u0430lo2/ipa"',
    '"halo2/ipa\\0"',
    '"halo2/ipa:production-ready"',
    '"stark/fri/S.e.c.u.r.i.t.yReviewPassed"',
  ]) {
    assert.ok(
      csharpVerifyingKeyBackendTests.includes(requiredCsharpVerifierBackendFixture),
      `Kagemusha C# verifier backend tests must keep fixture ${requiredCsharpVerifierBackendFixture}`,
    );
  }
  const csharpToriiTests = source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/ToriiClientTests.cs");
  for (const requiredCsharpIdentifierReceiptTest of [
    "ResolveIdentifierAsyncRejectsPaddedSignatureReceiptFields",
    "ResolveIdentifierAsyncRejectsNonExactPolicyIdBeforeDispatch",
    "ResolveIdentifierAsyncRejectsNonExactTopLevelPolicyId",
    "ResolveIdentifierAsyncRejectsNonExactSignaturePayloadPolicyIds",
    "ResolveIdentifierAsyncAcceptsExactProofAttestationReceipt",
    "ResolveIdentifierAsyncRejectsNonExactAttestationSelectors",
    "ResolveIdentifierAsyncAcceptsExactNestedReceiptPayloadFields",
    "ResolveIdentifierAsyncRejectsNonExactNestedReceiptPayloadFields",
    "GetIdentifierPoliciesAsyncRejectsNonExactPolicySummaryFields",
  ]) {
    assert.ok(
      csharpToriiTests.includes(requiredCsharpIdentifierReceiptTest),
      `Kagemusha C# Torii tests must keep ${requiredCsharpIdentifierReceiptTest}`,
    );
  }
  for (const requiredCsharpIdentifierReceiptField of [
    "identifier resolve response.signature_payload.attestation.proof_b64",
    "identifier resolve response.signature_payload.payload.policy_id",
    "identifier resolve response.signature_payload.payload.execution.program_id",
    "identifier resolve response.signature_payload.payload.account_id",
    "identifier resolve response.signature_payload.payload.receipt_hash",
    "identifier resolve response.signature_payload.payload.execution.executed_at_ms",
    "identifier policies response.items[0].policy_id",
    "identifier policies response.items[0].resolver_public_key",
  ]) {
    assert.ok(
      csharpToriiTests.includes(requiredCsharpIdentifierReceiptField),
      `Kagemusha C# Torii tests must keep ${requiredCsharpIdentifierReceiptField}`,
    );
  }
  assert.match(
    pythonRunner,
    /PYTHON_OVERRIDE="\$\{KAGEMUSHA_RECURSIVE_SPEND_PYTHON_BIN:-\}"[\s\S]*resolve_python_311_bin\(\)[\s\S]*python3\.11[\s\S]*PYTHON_BIN="\$\(resolve_python_311_bin\)"/,
    "Kagemusha Python SDK runner must keep the documented Python override variable",
  );
  assert.match(
    pythonRunner,
    /export VIRTUAL_ENV="\$\{VENV_DIR\}"[\s\S]*export PATH="\$\{VENV_DIR\}\/bin:\$\{PATH\}"[\s\S]*"\$\{VENV_DIR\}\/bin\/python" -m maturin develop --release[\s\S]*tests\/test_nexus_app\.py[\s\S]*tests\/offline_cash_test\.py[\s\S]*tests\/testconnect_codec\.py[\s\S]*tests\/test_address_format\.py/,
    "Kagemusha Python SDK runner must activate the selected venv before maturin and run Nexus wallet signature, Connect codec, and offline cash issuer-key exactness tests",
  );
  const pythonConnectTests = source("python/iroha_python/tests/testconnect_codec.py");
  for (const requiredPythonConnectTest of [
    "test_connect_codec_fails_closed_when_native_unavailable",
    "test_generate_connect_sid_matches_deterministic_vector",
    "test_generate_connect_sid_rejects_malformed_inputs",
    "test_create_connect_session_preview_builds_canonical_uris",
    "test_bootstrap_connect_preview_session_registers_and_extracts_tokens",
    "test_bootstrap_connect_preview_session_can_skip_registration",
    "test_bootstrap_connect_preview_session_rejects_bad_options_before_registration",
    "test_bootstrap_connect_preview_session_rejects_missing_tokens",
    "test_connect_sign_result_ok_rejects_confusable_algorithms",
    "test_connect_sign_result_ok_from_dict_rejects_padded_algorithm",
    "test_connect_control_approve_rejects_confusable_algorithms",
    "test_connect_control_approve_from_dict_rejects_padded_algorithm",
    "test_native_loader_rejects_wrong_python_framework",
  ]) {
    assert.ok(
      pythonConnectTests.includes(requiredPythonConnectTest),
      `Kagemusha Python Connect tests must keep ${requiredPythonConnectTest}`,
    );
  }
  const pythonConnectExactnessBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-python-connect-test-exactness":'),
    guard.indexOf('if mode == "--negative-control-python-sdk-canonical-request-test-filter-script":'),
  );
  assert.match(
    pythonConnectExactnessBranch,
    /test_generate_connect_sid_matches_deterministic_vector[\s\S]*?test_generate_connect_sid_accepts_random_vector[\s\S]*?test_bootstrap_connect_preview_session_rejects_bad_options_before_registration[\s\S]*?test_bootstrap_connect_preview_session_accepts_bad_options_before_registration[\s\S]*?test_connect_sign_result_ok_rejects_confusable_algorithms[\s\S]*?test_connect_sign_result_ok_accepts_confusable_algorithms/u,
    "Python Connect exactness negative control must mutate SID, bootstrap, and signature exactness tests",
  );
  assert.match(
    pythonConnectExactnessBranch,
    /except\s+ParityError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Python Connect exactness test drift was not detected"\)/u,
    "Python Connect exactness negative control must only pass after detecting injected drift",
  );
  assert.match(
    pythonRunner,
    /python\/iroha_torii_client\/tests\/test_client\.py::test_canonical_request_auth_rejects_padded_fields_before_send/,
    "Kagemusha Python SDK runner must exercise Torii canonical request auth exactness tests",
  );
  assert.match(
    pythonRunner,
    /python\/iroha_torii_client\/tests\/test_client\.py::test_identifier_resolution_receipt_matches_shared_vectors/,
    "Kagemusha Python SDK runner must exercise identifier receipt exactness tests",
  );
  assert.match(
    pythonRunner,
    /python\/iroha_torii_client\/tests\/test_client\.py::test_propose_multisig_rejects_malformed_response_fields/,
    "Kagemusha Python SDK runner must exercise multisig resolved account exactness tests",
  );
  const pythonMultisigResponseFilterBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-python-sdk-multisig-response-test-filter-script":'),
    guard.indexOf('if mode == "--negative-control-identifier-receipt-proof-base64-guard":'),
  );
  assert.match(
    pythonMultisigResponseFilterBranch,
    /test_propose_multisig_rejects_malformed_response_fields[\s\S]*?negative control rejected Python SDK multisig response test filter drift[\s\S]*?Python SDK multisig response test filter drift was not detected/u,
    "Python multisig response runner-filter negative control must mutate and detect the focused test filter",
  );
  assert.match(
    pythonRunner,
    /tests\/client_ledger_helpers_test\.py[\s\S]*zk_event_filters_reject_unsupported_backends_before_request[\s\S]*zk_verifying_key_event_filters_reject_malformed_names_before_request[\s\S]*zk_proof_event_filters_reject_malformed_hashes_before_request[\s\S]*zk_raw_event_filters_reject_malformed_privacy_matchers_before_request[\s\S]*zk_raw_event_filters_canonicalize_privacy_matchers_before_request/,
    "Kagemusha Python SDK runner must exercise Torii event-filter verifier/proof exactness tests",
  );
  const pythonCryptoAlgorithmTests = source("python/iroha_python/tests/crypto_algorithms_test.py");
  for (const requiredPythonCryptoExactnessTest of [
    "test_algorithm_labels_reject_empty_strings_across_public_api",
    "test_algorithm_labels_reject_surrounding_whitespace_across_public_api",
    "test_algorithm_labels_reject_empty_and_padded_native_inputs",
    "test_algorithm_labels_reject_control_and_confusable_native_inputs",
  ]) {
    assert.ok(
      pythonCryptoAlgorithmTests.includes(requiredPythonCryptoExactnessTest),
      `Kagemusha Python crypto tests must keep ${requiredPythonCryptoExactnessTest}`,
    );
  }
  for (const requiredPythonNativeCryptoCall of [
    "crypto_module._crypto.normalize_crypto_algorithm(label)",
    "crypto_module._crypto.generate_keypair(label)",
    "crypto_module._crypto.derive_keypair_from_seed(",
    "crypto_module._crypto.load_keypair(keypair.private_key, label)",
    "crypto_module._crypto.public_key_multihash(label, keypair.public_key, False)",
    "crypto_module._crypto.private_key_multihash(label, keypair.private_key, False)",
    "crypto_module._crypto.sign(label, keypair.private_key, payload)",
    "crypto_module._crypto.verify(label, keypair.public_key, payload, signature)",
  ]) {
    assert.ok(
      pythonCryptoAlgorithmTests.includes(requiredPythonNativeCryptoCall),
      `Kagemusha Python crypto tests must keep direct native exactness call ${requiredPythonNativeCryptoCall}`,
    );
  }
  assert.match(
    jsRunner,
    /NODE_OVERRIDE="\$\{KAGEMUSHA_RECURSIVE_SPEND_JS_SDK_NODE_BIN:-\}"[\s\S]*is_node_20_bin\(\)[\s\S]*resolve_node_20_bin\(\)[\s\S]*NODE_BIN="\$\(resolve_node_20_bin\)"/,
    "Kagemusha JavaScript SDK runner must keep the documented Node override variable",
  );
  assert.match(
    jsRunner,
    /NODE_VERSION="\$\("\$\{NODE_BIN\}" --version\)"/,
    "Kagemusha JavaScript SDK runner must print the selected Node version",
  );
  assert.match(
    jsRunner,
    /printf '%s\\n' "\$\{NODE_VERSION\}"[\s\S]*v20\.\*\) ;;/,
    "Kagemusha JavaScript SDK runner must reject non-Node-20 runtimes",
  );
  assert.match(
    jsRunner,
    /Kagemusha recursive spend\|Kagemusha record-backed\|Kagemusha \.\* SDK runner\|browser crypto exposes native-only helpers as safe stubs\|buildKagemusha\|privacy native availability probes build and verify with Norito request archives\|privacy native wrappers require binary Norito request archives\|privacy algorithm JS catalogs reject malformed internal review evidence\|fromAccount rejects control and Unicode-confusable curve algorithm aliases\|offline cash configuration snapshot requires cached issuer key and ABI\|canonical request signing: rejects padded auth fields\|streamEvents rejects unsupported production backend event filters before fetch\|streamEvents rejects malformed verifying key event names before fetch\|streamEvents rejects malformed proof event hashes before fetch\|ZK-ACE verifier-key references reject padded selector metadata\|privacy proof envelopes preserve pending production backend tags\|verifyIdentifierResolutionReceipt rejects adversarial receipt mutations\|encodeIdentifierResolutionReceiptPayload rejects non-exact execution tags\|encodeIdentifierResolutionReceiptAttestation rejects padded proof backend\|verifyIdentifierResolutionReceipt matches shared receipt vectors\|NexusAppClient rejects non-Ed25519 wallet signatures\|NexusAppClient accepts exact numeric and string Ed25519 signature algorithm tags\|ToriiClient attaches canonical signing headers for app endpoints\|ToriiClient canonical auth uses raw Node transport for UTF-8 account headers\|ToriiClient canonical auth rejects UTF-8 account headers when no supported transport is available\|ToriiClient canonical auth rejects non-byte private key arrays\|subscription plan and create endpoints send normalized payloads\|subscription action endpoints send normalized payloads\|getSubscription returns null on 404\|buildConnectWebSocketUrl rejects token query parameters\|buildConnectWebSocketUrl rejects endpoint host overrides\|buildConnectWebSocketUrl rejects endpoint protocol mismatches\|openConnectWebSocket injects Sec-WebSocket-Protocol when headers are unavailable\|openConnectWebSocket emits telemetry when allowInsecure is used\|resolveAliasByIndex enforces non-negative indices before issuing requests\|resolveAlias attaches canonical auth when provided\|lookupAliasesByAccount validates options before issuing requests[\s\S]*test\/address\.test\.js[\s\S]*test\/canonicalRequest\.test\.js[\s\S]*test\/connectWebSocket\.test\.js[\s\S]*test\/crypto\.browser\.test\.js[\s\S]*test\/instructionBuilders\.test\.js[\s\S]*test\/kagemushaFfiContractParity\.test\.js[\s\S]*test\/kagemushaRecursiveSpend\.test\.js[\s\S]*test\/nexusAppClient\.test\.js[\s\S]*test\/offlineCashLifecycle\.test\.js[\s\S]*test\/package_dist\.test\.js[\s\S]*test\/privacyCatalogParity\.test\.js[\s\S]*test\/privacyNative\.test\.js[\s\S]*test\/toriiCanonicalAuth\.test\.js[\s\S]*test\/toriiClient\.identifier\.test\.js[\s\S]*test\/toriiClient\.isoAlias\.test\.js[\s\S]*test\/toriiClient\.test\.js[\s\S]*test\/toriiSubscriptions\.test\.js[\s\S]*test\/transactionBuilder\.test\.js/,
    "Kagemusha JavaScript SDK runner must exercise recursive spend, address exactness, Nexus wallet signature exactness, offline cash issuer-key exactness, canonical request auth exactness, Torii event-filter exactness, Torii canonical auth/subscription/Connect WebSocket/ISO alias exactness, verifier-key exactness, identifier receipt exactness, privacy catalog, privacy-native, package-dist, transaction-builder, and runtime-gate meta tests",
  );
});
