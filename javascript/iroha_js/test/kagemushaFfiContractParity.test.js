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
      "ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
      "NativeTransitionProfileInit",
      "NativeTransitionProfileAppend",
      "NativeLineageAppendBoundary",
      "NativeCompactPaymentToken",
      "NativeRecursiveAggregationProofBundle",
    ],
    "C# SDK",
  );
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
      "schema must match RedeemKagemushaRecursive",
      "checksum is invalid",
      "must not be compressed",
      "valid KagemushaTransfer Norito archive",
      "bytesBase64 must be canonical standard base64",
      "buildKagemushaInstructionTransaction wraps one archive instruction",
      "buildKagemushaRecursiveRedeemTransaction derives instruction before signing",
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
        "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION = 7",
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
      "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION = 7",
      "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
      "is_kagemusha_recursive_compact_payment_token_prover_available",
      "is_kagemusha_recursive_compact_payment_token_verifier_available",
      "is_kagemusha_recursive_spend_compact_payment_token_projection_available",
      "is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available",
      "_RECURSIVE_COMPACT_TOKEN_METHOD",
      '"kagemusha_prove_verified_recursive_compact_payment_token"',
      '"_with_records_and_pallas_open_envelopes"',
      "_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD",
      '"kagemusha_verify_recursive_compact_payment_token"',
      "_RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD",
      '"kagemusha_recursive_spend_compact_payment_token_from_bundle"',
      "_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD",
      '"kagemusha_verify_recursive_spend_compact_payment_token_projection"',
      "_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_AT_HEIGHT_METHOD",
      '"kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"',
      "globals()[_RECURSIVE_COMPACT_TOKEN_METHOD]",
      "globals()[_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD]",
      "globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD]",
      "globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD]",
      '("archive", "norito", "probe")',
      '_assert_kagemusha_norito_archive(compact_token, "compact_token_archive")',
      '_archive_bytes_named(',
      "recursive_compact_verifier_keys_archive,",
      '_assert_kagemusha_norito_archive(',
      '_assert_kagemusha_norito_archive(verifier_record, "verifier_record_archive")',
      '_norito_archive_bytes_named(bundle_archive, "bundle_archive")',
      "block_height must be non-negative",
      "returned non-boolean result",
    ],
    "Python recursive compact verifier surface",
  );
  assertContainsAll(
    source("python/iroha_python/src/iroha_python/__init__.py"),
    [
      "is_kagemusha_recursive_compact_payment_token_prover_available",
      "is_kagemusha_recursive_compact_payment_token_verifier_available",
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
      'name = "kagemusha_verify_recursive_spend_compact_payment_token_projection"',
      'name = "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"',
      "is_kagemusha_recursive_compact_unavailable_error",
      "kagemusha_recursive_spend_compact_projection_verifier_python_rejects_malformed_inputs",
      "sentinel-spoofed recursive compact token must reject",
    ],
    "Python PyO3 recursive compact exports",
  );

  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift"),
    [
      "requiredBridgeAbiVersion: UInt32 = 7",
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
      "REQUIRED_BRIDGE_ABI_VERSION: Int = 7",
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
      "RecursiveCompactRequiredBridgeAbiVersion = 7",
      "IsRecursiveCompactPaymentTokenVerifierAvailable",
      "IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable",
      "public static bool VerifyRecursiveCompactPaymentToken(",
      "ReadOnlySpan<byte> recursiveCompactVerifierKeysArchive",
      "ReadOnlySpan<byte> recursiveCompactKeyArtifactsArchive",
      "public static bool VerifyRecursiveSpendCompactPaymentTokenProjection(",
      "public static KagemushaRecursiveCompactPaymentTokenArchive RecursiveSpendCompactPaymentTokenFromBundle",
      "TryProbeRecursiveSpendCompactPaymentTokenProjectionVerifierSymbol",
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
      "Recursive compact verifier keys archive must be a valid Norito archive",
      "VerifyRecursiveSpendCompactPaymentTokenProjection",
      "RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge",
      "RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
      "RecursiveSpendCompactProjectionRejectsInvalidBundleBeforeLoadingNativeBridge",
      "RecursiveSpendCompactProjectionVerifierRejectsInvalidInputsBeforeLoadingNativeBridge",
      "RecursiveSpendNativeReadBridgeOutputRejectsMalformedNoritoSuccessOutput",
      "RecursiveSpendNativeReadBridgeOutputRejectsEmptyPayloadNoritoSuccessOutput",
      "RecursiveSpendNativeReadBridgeOutputReturnsValidNoritoSuccessOutput",
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
      "prove_verified_kagemusha_compact_payment_token_from_record_bundle",
      "prove_verified_kagemusha_recursive_aggregation_proof_bundle_from_record_bundle_and_pallas_open_envelope_archive",
      "KAGEMUSHA_FOLDED_CIRCUIT_ID",
      "KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID",
    ],
    "Node record-backed Kagemusha prover exports",
  );

  for (const relative of ["javascript/iroha_js/src/crypto.js", "javascript/iroha_js/dist/crypto.js"]) {
    assertContainsAll(
      source(relative),
      [
        "isKagemushaCompactPaymentTokenNativeAvailable",
        "isKagemushaRecursiveAggregationProofBundleNativeAvailable",
        ...REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS,
        'typeof native.kagemushaProveVerifiedCompactPaymentTokenWithRecords !== "function"',
        "native.kagemushaProveVerifiedCompactPaymentTokenWithRecords(",
        "native.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(",
        "toOwnedKagemushaArchiveBuffer",
        'const recordBundle = toOwnedKagemushaArchiveBuffer(',
        'const pallasOpenEnvelopes = toOwnedKagemushaArchiveBuffer(',
        '"recordBundleArchive"',
        '"pallasOpenEnvelopesArchive"',
        "Kagemusha compact payment-token prover requires native bridge ABI 6",
        "Kagemusha recursive aggregation proof-bundle prover requires native bridge ABI 6",
      ],
      `${relative} record-backed Kagemusha wrappers`,
    );
  }
  for (const relative of ["javascript/iroha_js/src/crypto.browser.js", "javascript/iroha_js/dist/crypto.browser.js"]) {
    assertContainsAll(
      source(relative),
      [
        "isKagemushaCompactPaymentTokenNativeAvailable",
        "isKagemushaRecursiveAggregationProofBundleNativeAvailable",
        ...REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS,
        'unsupported("kagemushaProveVerifiedCompactPaymentTokenWithRecords")',
      ],
      `${relative} record-backed Kagemusha browser stubs`,
    );
  }
  for (const relative of ["javascript/iroha_js/src/index.js", "javascript/iroha_js/dist/index.js"]) {
    assertContainsAll(
      source(relative),
      REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS,
      `${relative} record-backed Kagemusha exports`,
    );
  }
  assertContainsAll(
    source("javascript/iroha_js/index.d.ts"),
    [
      "isKagemushaCompactPaymentTokenNativeAvailable(): boolean",
      "isKagemushaRecursiveAggregationProofBundleNativeAvailable(): boolean",
      "kagemushaProveVerifiedCompactPaymentTokenWithRecords(",
      "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(",
    ],
    "JavaScript record-backed Kagemusha TypeScript declarations",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/kagemushaRecursiveSpend.test.js"),
    [
      "Kagemusha record-backed JS builders probe availability and validate native output",
      "recordBundleArchive must be a valid Norito archive",
      "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
      "returned invalid Norito archive",
      "returned empty Norito payload",
    ],
    "JavaScript record-backed Kagemusha runtime tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/crypto.browser.test.js"),
    [
      "browser build must not expose native compact-token prover",
      "browser build must not expose native recursive aggregation prover",
      ...REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS,
    ],
    "JavaScript record-backed Kagemusha browser tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "isKagemushaCompactPaymentTokenNativeAvailable",
      "isKagemushaRecursiveAggregationProofBundleNativeAvailable",
      ...REQUIRED_RECORD_BACKED_KAGEMUSHA_JS_METHODS,
    ],
    "JavaScript package record-backed Kagemusha exports",
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
        "const KAGEMUSHA_MAX_BRIDGE_ABI_VERSION = 0xffff_ffff",
        "Number.isSafeInteger(version)",
        "version >= 0",
        "version <= KAGEMUSHA_MAX_BRIDGE_ABI_VERSION",
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
      "KAGEMUSHA_MAX_BRIDGE_ABI_VERSION = 0xFFFF_FFFF",
      "isinstance(version, bool)",
      "not isinstance(version, int)",
      "version < 0",
      "version > KAGEMUSHA_MAX_BRIDGE_ABI_VERSION",
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
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-compact-open",
    "--negative-control-abi7-core-contract-open",
    "--negative-control-abi7-bridge-unavailable-mapping",
    "--negative-control-abi7-offline-doc-one-hop-boundary",
    "--negative-control-compact-key-release-tooling",
    "--negative-control-compact-key-evidence",
    "--negative-control-compact-key-evidence-path-aliases",
    "--negative-control-compact-key-command-canonical",
    "--negative-control-compact-key-scalar-types",
    "--negative-control-compact-key-timestamp-raw",
    "--negative-control-compact-key-evidence-filename",
    "--negative-control-compact-key-closed-schema",
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
  const branchSpecs = [
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
      "--negative-control-release-bundle-compact-generator-log-inventory",
      /"compact_key_generator_log"[\s\S]*?"compactKeyGeneratorLogDisabled"/u,
      "Kagemusha release bundle compact generator log inventory",
    ],
  ];
  for (const [mode, mutationPattern, label] of branchSpecs) {
    const branch = readinessBranch(mode);
    assert.match(branch, /run_negative_control\(/u, `${label} negative control must use the shared runner`);
    assert.match(branch, mutationPattern, `${label} negative control must mutate the guarded source text`);
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

test("recursive Kagemusha policy negative controls pin lineage accumulator coverage", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-core-lineage-profile-split",
    "--negative-control-core-proof-chain-accumulator",
    "--negative-control-core-fixed-window-table-base-accumulator",
    "--negative-control-core-append-boundary-accumulator",
    "--negative-control-core-previous-accumulator-boundary",
    "--negative-control-core-resulting-accumulator-boundary",
    "--negative-control-core-append-boundary-digest-match",
    "--negative-control-core-append-boundary-context-matches",
    "--negative-control-core-append-digest-unchecked-surface",
    "--negative-control-core-append-digest-wrapper-bypass",
    "--negative-control-core-append-boundary-profile-comparison",
    "--negative-control-data-model-proof-public-input-circuit-binding",
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
  assert.match(
    profileSplitBranch,
    /Reserved-lineage one-hop and append verifier records must coexist under distinct circuit ids[\s\S]*?Reserved-lineage verifier records must coexist/u,
    "Reserved-lineage profile split negative control must mutate the guarded coverage",
  );
  assert.match(
    profileSplitBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "Reserved-lineage profile split negative control must validate the mutated text snapshot",
  );
  assert.match(
    profileSplitBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: Reserved-lineage profile split drift was not detected"\)/u,
    "Reserved-lineage profile split negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    profileSplitBranch,
    /\n    raise SystemExit\(0\)\n    raise SystemExit\("negative control failed: Reserved-lineage profile split drift was not detected"\)/u,
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
    guard.indexOf('if mode == "--negative-control-core-resulting-accumulator-boundary":'),
  );
  assert.match(
    previousAccumulatorBoundaryBranch,
    /field: "append_boundary\.previous_accumulator_digest"[\s\S]*?field: "append_boundary\.previous_accumulator_digest_unchecked"/u,
    "previous accumulator boundary negative control must mutate the guarded coverage",
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
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append-boundary context match drift was not detected"\)/u,
    "append-boundary context match negative control must only pass after detecting injected drift",
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
      /extra compact Pallas opening must reject before proving[\s\S]*?extra compact Pallas opening may return unavailable[\s\S]*?height-aware detached compact Pallas archive must reject before proving[\s\S]*?height-aware detached compact Pallas archive may return unavailable[\s\S]*?height-aware extra compact Pallas opening must reject before proving[\s\S]*?height-aware extra compact Pallas opening may return unavailable[\s\S]*?missing compact Pallas opening must reject before proving[\s\S]*?missing compact Pallas opening may return unavailable[\s\S]*?height-aware missing compact Pallas opening must reject before proving[\s\S]*?height-aware missing compact Pallas opening may return unavailable[\s\S]*?duplicated multi-hop compact Pallas archive must reject before proving[\s\S]*?duplicated multi-hop compact Pallas archive may return unavailable[\s\S]*?height-aware duplicated multi-hop compact Pallas archive must reject before proving[\s\S]*?height-aware duplicated multi-hop compact Pallas archive may return unavailable[\s\S]*?reordered multi-hop compact Pallas archive must reject before proving[\s\S]*?reordered multi-hop compact Pallas archive may return unavailable[\s\S]*?height-aware reordered multi-hop compact Pallas archive must reject before proving[\s\S]*?height-aware reordered multi-hop compact Pallas archive may return unavailable/u,
      "core recursive compact Pallas opening count",
    ],
    [
      "--negative-control-core-recursive-compact-pallas-metadata",
      /forged multi-hop compact Pallas metadata must reject before proving[\s\S]*?forged multi-hop compact Pallas metadata may return unavailable[\s\S]*?height-aware forged multi-hop compact Pallas metadata must reject before proving[\s\S]*?height-aware forged multi-hop compact Pallas metadata may return unavailable/u,
      "core recursive compact Pallas metadata",
    ],
    [
      "--negative-control-core-recursive-compact-cid-spoof-key",
      /CID-spoofed ABI-7 compact verifier key must reject[\s\S]*?CID-spoofed ABI-7 compact verifier key may pass/u,
      "core recursive compact CID-spoof key",
    ],
    [
      "--negative-control-core-recursive-spend-compact-projection-token",
      /one-hop side scalar projection splice must reject[\s\S]*?one-hop side scalar projection splice may pass/u,
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
    "--negative-control-python-lineage-key-package-binding",
    "--negative-control-csharp-lineage-key-package-binding",
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
    "--negative-control-js-lineage-readonly-declarations",
    "--negative-control-sdk-archive-input-copy",
    "--negative-control-sdk-lineage-proving-key-copy",
    "--negative-control-sdk-helper-surface",
    "--negative-control-sdk-readme-boundary",
    "--negative-control-sdk-readme-proof-chain-accumulator",
    "--negative-control-offline-doc-native-owned-accumulator-boundary",
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
    "--negative-control-js-package-dist-declaration-sweep",
    "--negative-control-js-package-dist-nexus-declaration-sweep",
    "--negative-control-js-package-dist-kotodama-declaration-sweep",
    "--negative-control-js-dts-recursive-compact-key-package",
    "--negative-control-python-recursive-compact-root-export",
    "--negative-control-recursive-spend-compact-projection-surface",
    "--negative-control-js-compact-projection-block-height-validation",
    "--negative-control-python-recursive-spend-compact-projection-root-export",
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
    "--negative-control-jvm-sdk-jdk21-script",
    "--negative-control-jvm-sdk-java-home-override-script",
    "--negative-control-jvm-sdk-java-home-reject-script",
    "--negative-control-jvm-recursive-compact-verifier-availability",
    "--negative-control-jvm-recursive-compact-shape-classifier",
    "--negative-control-jvm-sdk-android-harness-script",
    "--negative-control-jvm-sdk-test-order-workflow",
    "--negative-control-jvm-sdk-needs-workflow",
    "--negative-control-swift-sdk-job-workflow",
    "--negative-control-swift-sdk-runner-workflow",
    "--negative-control-swift-sdk-parse-workflow",
    "--negative-control-swift-sdk-uc4-skip",
    "--negative-control-swift-lineage-data-copy",
    "--negative-control-swift-recursive-compact-verifier-bool",
    "--negative-control-swift-recursive-compact-verifier-availability",
    "--negative-control-swift-kagemusha-native-output-cap",
    "--negative-control-swift-kagemusha-instruction-transaction-builder",
    "--negative-control-swift-sdk-version-script",
    "--negative-control-swift-sdk-override-script",
    "--negative-control-swift-sdk-needs-workflow",
    "--negative-control-csharp-sdk-job-workflow",
    "--negative-control-csharp-sdk-setup-workflow",
    "--negative-control-csharp-sdk-dotnet-version-workflow",
    "--negative-control-csharp-sdk-setup-order-workflow",
    "--negative-control-csharp-sdk-dotnet-version-script",
    "--negative-control-csharp-sdk-dotnet-override-script",
    "--negative-control-csharp-sdk-dotnet-major-script",
    "--negative-control-csharp-sdk-native-bridge-script",
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
    "--negative-control-js-sdk-test-workflow",
    "--negative-control-js-sdk-install-order-workflow",
    "--negative-control-js-sdk-test-order-workflow",
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
    guard.indexOf('if mode == "--negative-control-swift-lineage-key-package-binding":'),
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
    guard.indexOf('if mode == "--negative-control-js-lineage-readonly-declarations":'),
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
  const sdkReadmeRecursiveCompactUnavailableBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-readme-recursive-compact-unavailable":'),
    guard.indexOf('if mode == "--negative-control-sdk-readme-compact-projection-verifier":'),
  );
  assert.match(
    sdkReadmeRecursiveCompactUnavailableBranch,
    /reserved ABI-7 state[\s\S]*?ABI-7 native state[\s\S]*?run_checks\(mutated\)/u,
    "SDK README recursive compact negative control must mutate the ABI-7 one-hop boundary",
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
    guard.indexOf('if mode == "--negative-control-offline-doc-native-owned-accumulator-boundary":'),
  );
  assert.match(
    sdkReadmeProofChainAccumulatorBranch,
    /previous recursive proof bytes and per-hop accumulator[\s\S]*?native-owned accumulator digests[\s\S]*?append-boundary[\s\S]*?scalar-projection[\s\S]*?previous\/resulting accumulator digests[\s\S]*?must not derive, supply, or patch accumulator state[\s\S]*?optional SDK metadata[\s\S]*?run_checks\(mutated\)/u,
    "SDK README proof-chain accumulator negative control must mutate the accumulator boundary",
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
  const offlineDocAccumulatorBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-offline-doc-native-owned-accumulator-boundary":'),
    guard.indexOf('if mode == "--negative-control-offline-doc-instruction-transaction-surface":'),
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
  const sdkReadmeCompactProjectionVerifierBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-sdk-readme-compact-projection-verifier":'),
    guard.indexOf('if mode == "--negative-control-sdk-readme-stale-future-lineage":'),
  );
  assert.match(
    sdkReadmeCompactProjectionVerifierBranch,
    /verifyRecursiveSpendCompactPaymentTokenProjection\(compactTokenArchive:verifierRecordArchive:blockHeight:\)[\s\S]*?verifyRecursiveSpendCompactPaymentTokenProjection\(compactTokenArchive:verifierRecordArchive:\)[\s\S]*?run_checks\(mutated\)/u,
    "SDK README compact projection verifier negative control must mutate the Swift API signature",
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
    guard.indexOf('if mode == "--negative-control-js-package-dist-declaration-sweep":'),
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
    /kagemushaRecursiveSpendCompactPaymentTokenFromBundle[\s\S]*?kagemushaRecursiveSpendCompactPaymentTokenFromBundleMissing/u,
    "recursive spend compact projection negative control must mutate the JS projection API",
  );
  assert.match(
    recursiveSpendCompactProjectionSurfaceBranch,
    /mutated_texts\s*=\s*dict\(texts\)[\s\S]*?mutated_texts\[target\]\s*=\s*mutated[\s\S]*?run_checks\(mutated_texts\)/u,
    "recursive spend compact projection negative control must validate the mutated text snapshot",
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
  const kagemushaProbeRejectionShapeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-kagemusha-probe-rejection-shape":'),
    guard.indexOf("if mode:\n    raise SystemExit"),
  );
  assert.match(
    kagemushaProbeRejectionShapeBranch,
    /mutated\s*=\s*dict\(texts\)[\s\S]*?mutated\[target\]\s*=\s*updated[\s\S]*?run_checks\(mutated\)/u,
    "Kagemusha probe rejection shape negative control must validate the mutated text snapshot",
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
    guard.indexOf('if mode == "--negative-control-swift-sdk-version-script":'),
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
  const swiftInstructionTransactionBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-swift-kagemusha-instruction-transaction-builder":'),
    guard.indexOf('if mode == "--negative-control-swift-sdk-version-script":'),
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
  const jsInstructionTransactionBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-kagemusha-instruction-transaction-builder":'),
    guard.indexOf('if mode == "--negative-control-python-lineage-key-package-binding":'),
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
  const jsRunner = source("ci/check_kagemusha_recursive_spend_js_sdk.sh");
  const pythonRunner = source("ci/check_kagemusha_recursive_spend_python_sdk.sh");
  const swiftRunner = source("ci/check_kagemusha_recursive_spend_swift_sdk.sh");
  const csharpRunner = source("ci/check_kagemusha_recursive_spend_csharp_sdk.sh");
  const jvmRunner = source("ci/check_kagemusha_recursive_spend_jvm_sdk.sh");
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
    swiftRunner,
    /SWIFTC_BIN="\$\{KAGEMUSHA_RECURSIVE_SPEND_SWIFTC_BIN:-swiftc\}"/,
    "Kagemusha Swift SDK runner must keep the documented swiftc override variable",
  );
  assert.match(
    csharpRunner,
    /DOTNET_BIN="\$\{KAGEMUSHA_RECURSIVE_SPEND_DOTNET_BIN:-dotnet\}"/,
    "Kagemusha C# SDK runner must keep the documented dotnet override variable",
  );
  assert.match(
    pythonRunner,
    /PYTHON_OVERRIDE="\$\{KAGEMUSHA_RECURSIVE_SPEND_PYTHON_BIN:-\}"[\s\S]*resolve_python_311_bin\(\)[\s\S]*python3\.11[\s\S]*PYTHON_BIN="\$\(resolve_python_311_bin\)"/,
    "Kagemusha Python SDK runner must keep the documented Python override variable",
  );
  assert.match(
    pythonRunner,
    /export VIRTUAL_ENV="\$\{VENV_DIR\}"[\s\S]*export PATH="\$\{VENV_DIR\}\/bin:\$\{PATH\}"[\s\S]*"\$\{VENV_DIR\}\/bin\/python" -m maturin develop --release/,
    "Kagemusha Python SDK runner must activate the selected venv before maturin",
  );
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
    /Kagemusha recursive spend\|Kagemusha record-backed\|Kagemusha \.\* SDK runner\|browser crypto exposes native-only helpers as safe stubs[\s\S]*test\/crypto\.browser\.test\.js[\s\S]*test\/kagemushaFfiContractParity\.test\.js[\s\S]*test\/kagemushaRecursiveSpend\.test\.js[\s\S]*test\/package_dist\.test\.js/,
    "Kagemusha JavaScript SDK runner must exercise recursive spend, package-dist, and runtime-gate meta tests",
  );
});
