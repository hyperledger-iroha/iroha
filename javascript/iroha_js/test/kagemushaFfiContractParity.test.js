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
      "NativeTransitionProfileInit",
      "NativeTransitionProfileAppend",
      "NativeLineageAppendBoundary",
    ],
    "C# SDK",
  );
});

test("recursive Kagemusha ABI-6 availability probes require transition-profile and boundary helpers", () => {
  assertContainsAll(
    source("javascript/iroha_js/src/crypto.js"),
    [
      '"kagemushaRecursiveSpendTransitionProfileInit"',
      '"kagemushaRecursiveSpendTransitionProfileAppend"',
      '"kagemushaRecursiveSpendLineageAppendBoundary"',
    ],
    "JavaScript availability probe",
  );
  assertContainsAll(
    source("python/iroha_python/src/iroha_python/kagemusha.py"),
    [
      '"kagemusha_recursive_spend_transition_profile_init"',
      '"kagemusha_recursive_spend_transition_profile_append"',
      '"kagemusha_recursive_spend_lineage_append_boundary"',
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
    ],
    "Swift availability probe",
  );
  assertContainsAll(
    source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"),
    [
      "expectIllegalArgumentProbe(() -> nativeTransitionProfileInit(new byte[0]))",
      "expectIllegalArgumentProbe(() -> nativeTransitionProfileAppend(new byte[0]))",
      "expectIllegalArgumentProbe(() -> nativeLineageAppendBoundary(new byte[0]))",
    ],
    "Android Java availability probe",
  );
  assertContainsAll(
    source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"),
    [
      "expectIllegalArgumentProbe { nativeTransitionProfileInit(ByteArray(0)) }",
      "expectIllegalArgumentProbe { nativeTransitionProfileAppend(ByteArray(0)) }",
      "expectIllegalArgumentProbe { nativeLineageAppendBoundary(ByteArray(0)) }",
    ],
    "Kotlin availability probe",
  );
  assertContainsAll(
    source("csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs"),
    [
      "Probe(NativeTransitionProfileInit)",
      "Probe(NativeTransitionProfileAppend)",
      "Probe(NativeLineageAppendBoundary)",
    ],
    "C# availability probe",
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

test("recursive Kagemusha SDK parity negative controls fail when drift is undetected", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_sdk_parity.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control",
    "--negative-control-workflow",
    "--negative-control-native-manifest-workflow",
    "--negative-control-js-browser-helper",
    "--negative-control-sdk-helper-surface",
    "--negative-control-sdk-readme-boundary",
    "--negative-control-sdk-readme-availability-surface",
    "--negative-control-sdk-readme-stale-future-lineage",
    "--negative-control-cross-sdk-helper-bodies",
    "--negative-control-sdk-negative-controls-workflow",
    "--negative-control-sdk-negative-controls-comment-workflow",
    "--negative-control-sdk-main-guard-workflow",
    "--negative-control-bytecode-workflow",
    "--negative-control-native-bridge-job-workflow",
    "--negative-control-native-bridge-runner-workflow",
    "--negative-control-native-bridge-cache-workflow",
    "--negative-control-native-bridge-test-workflow",
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
    "--negative-control-python-sdk-test-workflow",
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
    "--negative-control-jvm-sdk-test-order-workflow",
    "--negative-control-jvm-sdk-needs-workflow",
    "--negative-control-swift-sdk-job-workflow",
    "--negative-control-swift-sdk-runner-workflow",
    "--negative-control-swift-sdk-parse-workflow",
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

  const browserHelperBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-js-browser-helper":'),
    guard.indexOf('if mode == "--negative-control-sdk-helper-surface":'),
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
    /Kagemusha recursive spend\|Kagemusha \.\* SDK runner[\s\S]*test\/kagemushaFfiContractParity\.test\.js[\s\S]*test\/kagemushaRecursiveSpend\.test\.js[\s\S]*test\/package_dist\.test\.js/,
    "Kagemusha JavaScript SDK runner must exercise recursive spend, package-dist, and runtime-gate meta tests",
  );
});
