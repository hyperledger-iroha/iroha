import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import test from "node:test";
import { fileURLToPath } from "node:url";

const REPO_ROOT = fileURLToPath(new URL("../../..", import.meta.url));

// The first release has no compatibility export set. Only exact ABI-18 V2
// proof/protocol symbols and the governed V3 artifact lifecycle ship.
const REQUIRED_C_SYMBOLS = Object.freeze([]);

const REQUIRED_KAGEMUSHA_V2_PROOF_SYMBOLS = Object.freeze([
  "connect_norito_kagemusha_recursive_spend_init_v2",
  "connect_norito_kagemusha_recursive_spend_append_v2",
  "connect_norito_kagemusha_recursive_spend_redeem_change_v2",
  "connect_norito_kagemusha_recursive_spend_verify_v2",
  "connect_norito_kagemusha_recursive_spend_redeem_v2",
]);

const REQUIRED_KAGEMUSHA_V2_PROTOCOL_SYMBOLS = Object.freeze([
  "connect_norito_kagemusha_recursive_spend_capabilities_v1",
  "connect_norito_kagemusha_topup_finality_verify_v2",
  "connect_norito_kagemusha_topup_shield_build_unsigned_v2",
  "connect_norito_kagemusha_recursive_spend_topup_v2",
  "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2",
  "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2",
  "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2",
  "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2",
  "connect_norito_kagemusha_receiver_key_reference_v2",
  "connect_norito_kagemusha_recipient_output_derive_v2",
  "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
  "connect_norito_kagemusha_recipient_payment_request_create_v2",
  "connect_norito_kagemusha_recipient_payment_request_verify_v2",
  "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
  "connect_norito_kagemusha_request_authorization_create_v2",
  "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
  "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
  "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
  "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
  "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2",
  "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2",
  "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
  "connect_norito_kagemusha_recursive_spend_build_split_intent_v2",
  "connect_norito_kagemusha_recursive_spend_build_redemption_intent_v2",
  "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
]);

const REQUIRED_KAGEMUSHA_V2_NATIVE_SYMBOLS = Object.freeze([
  ...REQUIRED_KAGEMUSHA_V2_PROOF_SYMBOLS,
  ...REQUIRED_KAGEMUSHA_V2_PROTOCOL_SYMBOLS,
]);

const ABI18_V2_C_SYMBOLS = Object.freeze([
  "connect_norito_kagemusha_recursive_spend_init_v2",
  "connect_norito_kagemusha_recursive_spend_append_v2",
  "connect_norito_kagemusha_recursive_spend_verify_v2",
  "connect_norito_kagemusha_recursive_spend_redeem_v2",
  "connect_norito_kagemusha_recursive_spend_topup_v2",
  "connect_norito_kagemusha_recursive_spend_redeem_change_v2",
  "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2",
  "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2",
  "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2",
  "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2",
  "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2",
  "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2",
  "connect_norito_kagemusha_recursive_spend_build_split_intent_v2",
  "connect_norito_kagemusha_recursive_spend_build_redemption_intent_v2",
  "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
]);

const ADDITIVE_ABI18_V3_C_SYMBOLS = Object.freeze([
  "connect_norito_kagemusha_recursive_spend_capabilities_v1",
  "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
  "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
]);

const CURRENT_C_SYMBOLS = Object.freeze([
  ...REQUIRED_C_SYMBOLS,
  ...ABI18_V2_C_SYMBOLS,
  ...ADDITIVE_ABI18_V3_C_SYMBOLS,
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
  "kagemushaRecursiveSpendTopUp",
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
  "kagemusha_recursive_spend_topup",
]);

const REQUIRED_RECURSIVE_COMPACT_PYTHON_METHODS = Object.freeze([
  "kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes",
  "kagemusha_verify_recursive_compact_payment_token",
  "kagemusha_recursive_spend_compact_payment_token_from_bundle",
]);

const REQUIRED_HEADER_NEGATIVE_CONTROL_MODES = Object.freeze([
  "--negative-control-missing-recursive-header",
  "--negative-control-bad-recursive-signature",
  "--negative-control-bad-recursive-v2-signature",
  "--negative-control-missing-recursive-v2-export-pair",
  "--negative-control-missing-kagemusha-v2-protocol-export-pair",
  "--negative-control-bad-kagemusha-v2-receiver-key-signature",
  "--negative-control-bad-kagemusha-v2-verify-at-time-signature",
  "--negative-control-bad-kagemusha-v2-ack-create-signature",
  "--negative-control-bad-connect-norito-free-signature",
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
  for (const invalidVersion of ["7.0.404", "8.0.0", "8.0.010", "8.0.128-preview.1"]) {
    const tmp = mkdtempSync(`${tmpdir()}/iroha-dotnet-runner-`);
    const fakeDotnet = `${tmp}/dotnet`;
    try {
      writeFileSync(
        fakeDotnet,
        [
          "#!/usr/bin/env bash",
          "if [[ \"${1:-}\" == \"--version\" ]]; then",
          `  printf '%s\\n' '${invalidVersion}'`,
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

      assert.notEqual(result.status, 0, `${label} must reject SDK ${invalidVersion}`);
      assert.ok(
        result.stdout.split(/\r?\n/u).includes(invalidVersion),
        `${label} must print dotnet version evidence for ${invalidVersion}`,
      );
      assert.match(
        result.stderr,
        /stable canonical \.NET SDK 8\.0\.x.+non-zero patch/u,
        `${label} must explain the strict .NET 8 gate`,
      );
      assert.doesNotMatch(
        result.stderr,
        /unexpected fake dotnet invocation/u,
        `${label} must fail before dotnet test for ${invalidVersion}`,
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
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
      /connect_norito_bridge native bridge sha256: [0-9a-f]{64}/u,
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
  const modeList = Array.from(modes);
  assert.equal(
    new Set(modeList).size,
    modeList.length,
    `${label} workflow expected negative-control modes must not contain duplicates`,
  );
  for (const mode of modeList) {
    assert.match(
      workflow,
      new RegExp(`^\\s+${escapeRegExp(command)} ${escapeRegExp(mode)}$`, "m"),
      `${label} workflow must run ${mode}`,
    );
  }
}

function negativeControlModesFromWorkflowRequirements(script, command) {
  const pattern = new RegExp(
    `"${escapeRegExp(command)} (--negative-control-[a-z0-9-]+)"`,
    "gu",
  );
  return [...script.matchAll(pattern)].map(([, mode]) => mode);
}

test("workflow negative-control helper rejects duplicate expected modes", () => {
  const workflow = "          ci/check_fixture.sh --negative-control-fixture\n";
  assert.throws(
    () =>
      assertWorkflowRunsNegativeControlModes(
        workflow,
        "ci/check_fixture.sh",
        ["--negative-control-fixture", "--negative-control-fixture"],
        "fixture guard",
      ),
    /fixture guard workflow expected negative-control modes must not contain duplicates/u,
  );
});

function assertContainsAll(text, names, label) {
  for (const name of names) {
    assert.ok(text.includes(name), `${label} missing ${name}`);
  }
}

function assertPerMutationDetector(branch, description, label) {
  assert.match(
    branch,
    /detected_messages\s*=\s*\[\][\s\S]*?current = mutated(?:\[target\]|\.get\(target, read\(target\)\))[\s\S]*?detect_negative_control\([\s\S]*?finally:[\s\S]*?mutated\[target\]\s*=\s*current[\s\S]*?if not detected_messages:[\s\S]*?for detected_message in detected_messages:[\s\S]*?print\(detected_message\)[\s\S]*?raise SystemExit\(0\)/u,
    label,
  );
  if (description) {
    assert.match(
      branch,
      new RegExp(escapeRegExp(description), "u"),
      `${label}: missing detector description`,
    );
  }
}

function assertSameSet(actual, expected, label) {
  assert.deepEqual(
    [...actual].sort(),
    [...expected].sort(),
    `${label} drifted`,
  );
}

test("recursive Kagemusha ABI-18 C exports and shipped headers stay in parity", () => {
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

  assertSameSet(rustExports, CURRENT_C_SYMBOLS, "Rust recursive Kagemusha C exports");
  assertSameSet(headerDeclarations, CURRENT_C_SYMBOLS, "C header recursive Kagemusha declarations");
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
    [
      '"connect_norito_kagemusha_recursive_spend_capabilities_v1": c_signature(',
      '"connect_norito_kagemusha_recursive_spend_artifact_begin_v3": c_signature(',
      '"connect_norito_kagemusha_recursive_spend_artifact_write_v3": c_signature(',
      '"connect_norito_kagemusha_recursive_spend_artifact_finalize_v3": c_signature(',
      '"connect_norito_kagemusha_recursive_spend_artifact_cancel_v3": c_signature(',
      "required_recursive_ffi = set(expected_recursive_signatures)",
    ],
    "NoritoBridge ABI-18 capability signature inventory",
  );
  const xcframeworkSymbolBlock = source(
    "ci/check_kagemusha_recursive_spend_sdk_parity.sh",
  ).slice(
    source("ci/check_kagemusha_recursive_spend_sdk_parity.sh").indexOf(
      "REQUIRED_BRIDGE_SYMBOLS=(",
    ),
    source("ci/check_kagemusha_recursive_spend_sdk_parity.sh").indexOf(
      "BRIDGE_LIBS=(",
    ),
  );
  assertContainsAll(
    xcframeworkSymbolBlock,
    ADDITIVE_ABI18_V3_C_SYMBOLS,
    "NoritoBridge XCFramework ABI-18 capability symbols",
  );
  assertContainsAll(
    headerGuard,
    REQUIRED_HEADER_NEGATIVE_CONTROL_MODES,
    "NoritoBridge header guard negative controls",
  );
});

test("recursive Kagemusha ABI-18 native host and SDK method names stay in parity", () => {
  const rustBridge = source("crates/connect_norito_bridge/src/lib.rs");
  const header = source("crates/connect_norito_bridge/include/connect_norito_bridge.h");
  const headerGuard = source("ci/check_connect_norito_bridge_header.sh");
  const androidJavaProver = source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java");
  const kotlinProver = source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt");
  const swiftNativeBridgeCore = source("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift");
  const swiftRecursiveSpendV2 = source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift");
  const swiftRecursiveSpendV2Native = source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2Native.swift");
  const swiftNativeBridge = [
    swiftNativeBridgeCore,
    swiftRecursiveSpendV2,
    swiftRecursiveSpendV2Native,
  ].join("\n");
  const swiftV2Inventory = (name) => {
    const match = swiftRecursiveSpendV2.match(
      new RegExp(`public static let ${name} = \\[([\\s\\S]*?)\\n    \\]`, "u"),
    );
    assert.ok(match, `Swift V2 inventory missing ${name}`);
    return namesFromMatches(match[1], /"([^"]+)"/gu);
  };
  const swiftV2ProofInventory = swiftV2Inventory("requiredProofSymbols");
  const swiftV2ProtocolInventory = swiftV2Inventory("requiredProtocolSymbols");
  const swiftV2NativeInventory = [
    ...swiftV2ProofInventory,
    ...swiftV2ProtocolInventory,
  ];
  const kagemushaV2InventoryFamily = String.raw`connect_norito_kagemusha_(?:topup_(?:finality_verify|shield_build_unsigned)_v2|receiver_key_reference_v2|recipient_output_derive_v2|recipient_payment_request_(?:signing_bytes|create|verify)_v2|request_authorization_(?:signing_bytes|create)_v2|receiver_acknowledgement_(?:payload|signing_bytes|create|verify)_v2|recursive_spend_(?:capabilities_v1|(?:init|topup|append|redeem_change|verify|redeem|topup_unsigned_payload_digest|topup_finalize_request|redeem_unsigned_payload_digest|redeem_finalize_request|peer_payment_from_split|peer_payment_validate|bundle_summary|build_split_intent|build_redemption_intent)_v2|artifact_(?:begin|write|finalize|cancel|set_(?:install|is_installed|uninstall))_v3))`;
  const rustV2Inventory = new Set(
    namesFromMatches(
      rustBridge,
      new RegExp(
        `#\\[unsafe\\(no_mangle\\)\\]\\s*pub\\s+unsafe\\s+extern\\s+"C"\\s+fn\\s+(${kagemushaV2InventoryFamily})\\s*\\(`,
        "gu",
      ),
    ),
  );
  const headerV2Inventory = new Set(
    namesFromMatches(
      header,
      new RegExp(`int32_t\\s+(${kagemushaV2InventoryFamily})\\s*\\(`, "gu"),
    ),
  );

  assert.equal(
    REQUIRED_KAGEMUSHA_V2_PROOF_SYMBOLS.length,
    5,
    "ABI-18 must pin exactly five Kagemusha V2 proof symbols",
  );
  assert.equal(
    REQUIRED_KAGEMUSHA_V2_PROTOCOL_SYMBOLS.length,
    31,
    "ABI-18 must pin exactly thirty-one Kagemusha V2 protocol symbols",
  );
  assert.equal(
    new Set(swiftV2NativeInventory).size,
    swiftV2NativeInventory.length,
    "Swift V2 required native symbol inventory must not contain duplicates",
  );
  assertSameSet(
    new Set(swiftV2ProofInventory),
    REQUIRED_KAGEMUSHA_V2_PROOF_SYMBOLS,
    "Swift V2 required proof symbol inventory",
  );
  assertSameSet(
    new Set(swiftV2ProtocolInventory),
    REQUIRED_KAGEMUSHA_V2_PROTOCOL_SYMBOLS,
    "Swift V2 required protocol symbol inventory",
  );
  assertSameSet(
    rustV2Inventory,
    REQUIRED_KAGEMUSHA_V2_NATIVE_SYMBOLS,
    "Rust ABI-18 Kagemusha V2 export inventory",
  );
  assertSameSet(
    headerV2Inventory,
    REQUIRED_KAGEMUSHA_V2_NATIVE_SYMBOLS,
    "C header ABI-18 Kagemusha V2 declaration inventory",
  );
  assert.match(
    swiftRecursiveSpendV2,
    /requiredNativeSymbols = requiredProofSymbols \+ requiredProtocolSymbols/u,
    "Swift V2 availability inventory must combine proof and protocol symbols",
  );
  const swiftV2Availability = swiftNativeBridgeCore.slice(
    swiftNativeBridgeCore.indexOf("public var isKagemushaRecursiveSpendV2StubAvailable"),
    swiftNativeBridgeCore.indexOf("public var isPrivacyNativeAvailable"),
  );
  assert.match(
    swiftV2Availability,
    /KagemushaRecursiveSpend\.requiredNativeSymbols \+ \["connect_norito_free"\][\s\S]*?\.allSatisfy \{ hasKagemushaV2Symbol\(\$0\) \}/u,
    "Swift V2 availability must require every declared native symbol and the free function",
  );
  assert.doesNotMatch(
    swiftV2Availability,
    /unsafeBitCast/u,
    "Swift V2 availability must probe symbol presence without casting function pointers",
  );
  assert.match(
    rustBridge,
    /#\[unsafe\(no_mangle\)\]\s*pub\s+extern\s+"C"\s+fn\s+connect_norito_free\s*\(\s*ptr_\s*:\s*\*mut\s+c_uchar\s*,?\s*\)\s*\{/u,
    "Rust bridge must expose the exact mutable-byte connect_norito_free deallocator",
  );
  assert.match(
    header,
    /void\s+connect_norito_free\s*\(\s*uint8_t\s*\*\s*ptr\s*\)\s*;/u,
    "C header must declare the exact mutable-byte connect_norito_free deallocator",
  );
  assert.match(
    swiftRecursiveSpendV2Native,
    /typealias KagemushaV2FreeFn = @convention\(c\) \(UnsafeMutablePointer<UInt8>\?\) -> Void/u,
    "Swift V2 bridge must resolve connect_norito_free with the exact mutable-byte signature",
  );
  assertContainsAll(
    headerGuard,
    REQUIRED_KAGEMUSHA_V2_NATIVE_SYMBOLS,
    "NoritoBridge header guard ABI-18 Kagemusha V2 inventories",
  );
  assert.equal(
    REQUIRED_KAGEMUSHA_V2_NATIVE_SYMBOLS.length,
    36,
    "ABI-18 must pin the complete five-proof and thirty-one-protocol inventory",
  );
  assert.match(
    headerGuard,
    /expected_connect_norito_free_header_signature[\s\S]*expected_connect_norito_free_rust_signature[\s\S]*Rust connect_norito_free export has wrong signature[\s\S]*C header connect_norito_free declaration has wrong signature/u,
    "NoritoBridge header guard must reject Rust and C connect_norito_free signature drift",
  );
  assert.match(
    swiftRecursiveSpendV2Native,
    /typealias KagemushaV2KeyReferenceFn = @convention\(c\) \(\s*UInt8, UnsafePointer<UInt8>\?, CUnsignedLong,\s*UnsafeMutablePointer<UnsafeMutablePointer<UInt8>\?>\?, UnsafeMutablePointer<CUnsignedLong>\?\s*\) -> Int32/u,
    "Swift V2 receiver-key resolver must preserve the UInt8 algorithm ABI shape",
  );
  assert.match(
    swiftRecursiveSpendV2Native,
    /func kagemushaRecipientPaymentRequestVerifyV2\(\s*requestArchive: Data,\s*verifiedAtMilliseconds: UInt64\s*\) throws -> Data\?[\s\S]*?callKagemushaV2ArchiveAtTime\(\s*symbol: "connect_norito_kagemusha_recipient_payment_request_verify_v2",\s*archive: requestArchive,\s*milliseconds: verifiedAtMilliseconds\s*\)/u,
    "Swift V2 request verification must preserve the authoritative UInt64 time ABI shape",
  );
  assert.match(
    swiftRecursiveSpendV2Native,
    /func kagemushaReceiverAcknowledgementCreateV2\(\s*payloadArchive: Data,\s*signature: Data,\s*requestArchive: Data,\s*peerPaymentArchive: Data\s*\) throws -> Data\?[\s\S]*?callKagemushaV2FourArchives\(\s*symbol: "connect_norito_kagemusha_receiver_acknowledgement_create_v2",\s*first: payloadArchive,\s*second: signature,\s*third: requestArchive,\s*fourth: peerPaymentArchive\s*\)/u,
    "Swift V2 ACK creation must preserve all four archive arguments",
  );
  assert.doesNotMatch(
    swiftNativeBridgeCore,
    /kagemushaRecursiveSpendAppendV2Fn/u,
    "Swift bridge must not cache append_v2 under the one-archive C function type",
  );
  assert.doesNotMatch(
    swiftNativeBridgeCore,
    /func kagemushaRecursiveSpendAppendV2\(requestArchive: Data\)/u,
    "Swift bridge must not expose a one-archive append_v2 overload",
  );
  assert.match(
    swiftRecursiveSpendV2Native,
    /func kagemushaRecursiveSpendInitV2\(requestArchive: Data\) throws -> Data\?[\s\S]*?callKagemushaV2Archive\(\s*symbol: "connect_norito_kagemusha_recursive_spend_init_v2",\s*archive: requestArchive\s*\)/u,
    "Swift V2 init wrapper must retain the canonical flat one-archive shape",
  );
  assert.match(
    swiftRecursiveSpendV2Native,
    /func kagemushaRecursiveSpendAppendV2\(\s*requestArchive: Data,\s*recipientRequestArchive: Data,\s*verifiedAtMilliseconds: UInt64\s*\)/u,
    "Swift V2 append wrapper must retain the exact two-archive-plus-time shape",
  );

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
    swiftNativeBridge,
    REQUIRED_C_SYMBOLS,
    "Swift native bridge loaders",
  );
  assertContainsAll(
    swiftRecursiveSpendV2,
    [
      "initSpend",
      "appendSpend",
      "verifySpend",
      "redeemSpend",
      "topUpSpend",
      "ensureProofBackendAvailable",
      "validateFrame",
    ],
    "Swift V2 public recursive-spend surface",
  );

  assertContainsAll(
    androidJavaProver,
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
      "topUpSpend",
      "nativeTransitionProfileInit",
      "nativeTransitionProfileAppend",
      "nativeTopUpInstruction",
    ],
    "Android Java SDK",
  );
  assertContainsAll(
    kotlinProver,
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
      "topUpSpend",
      "nativeTransitionProfileInit",
      "nativeTransitionProfileAppend",
      "nativeTopUpInstruction",
      "nativeBuildPallasOpenEnvelopesArchive",
      "nativeBuildPreviousProofOpenEnvelopesArchive",
    ],
    "Kotlin JVM SDK",
  );
  assert.doesNotMatch(
    androidJavaProver,
    /nativeTopUpSpend/u,
    "Android Java SDK must not expose the retired init-request top-up JNI symbol",
  );
  assert.doesNotMatch(
    kotlinProver,
    /nativeTopUpSpend/u,
    "Kotlin JVM SDK must not expose the retired init-request top-up JNI symbol",
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
      "TopUp",
      "ProveVerifiedCompactPaymentTokenWithRecords",
      "BuildPallasOpenEnvelopesArchive",
      "BuildPreviousProofOpenEnvelopesArchive",
      "IsPallasOpenEnvelopeBuilderAvailable",
      "ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes",
      "NativeTransitionProfileInit",
      "NativeTransitionProfileAppend",
      "NativeLineageAppendBoundary",
      "NativeTopUp",
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
    "nativeTopUpInstruction",
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
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/AttestedOfflineNote.java",
      "Android Java Offline Note V2 proof metadata",
    ],
    [
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNote.kt",
      "Kotlin Offline Note proof metadata",
    ],
    [
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/AttestedOfflineNote.kt",
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
    source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/AttestedOfflineNoteTest.java"),
    [
      "padded proof backend should throw",
      'new AttestedOfflineNote.VerifyingKeyIdReference(" halo2/ipa ", "vk")',
      'new AttestedOfflineNote.VerifyingKeyIdReference("halo2/ipa", " vk ")',
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
    source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/AttestedOfflineNoteTest.kt"),
    [
      'AttestedOfflineNote.ProofBox("  ${AttestedOfflineNote.RECURSIVE_BACKEND}  ", byteArrayOf(1))',
      'AttestedOfflineNote.VerifyingKeyIdReference(backend = " halo2/ipa ", name = "vk")',
      'AttestedOfflineNote.VerifyingKeyIdReference(backend = "halo2/ipa", name = " vk ")',
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
});

test("Kagemusha mobile Offline Note V2 OpenVerifyEnvelope decoders stay wired", () => {
  for (const [relative, label] of [
    [
      "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/AttestedOfflineNoteHalo2Prover.java",
      "Android Java Offline Note V2 Halo2 prover",
    ],
    [
      "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/AttestedOfflineNoteHalo2Prover.java",
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
      "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/AttestedOfflineNoteTest.java",
      "Android Java Offline Note V2 tests",
    ],
    [
      "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/AttestedOfflineNoteTest.kt",
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
      "Kagemusha recursive spend helpers reject missing request archives before native calls",
      "archiveNames[fieldIndex]",
      "missing archive",
      "must be a Buffer, string, or ArrayBuffer view",
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
      "invalidArchive,\n              verifierRecordArchive,\n              9,",
      "compactTokenArchive,\n              invalidArchive,\n              9,",
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

test("Kagemusha C# recursive spend inputs require Norito archives", () => {
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
      "ValidateRedeemLineagePreflight",
      "lineageWitness is required for this bundle",
      "lineageVerifierRecord is required for reserved-lineage bundles",
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
      "RecursiveSpendNativeRedeemLineagePreflightRejectsMissingMaterialBeforeNativeBridge",
      "lineageWitness is required for this bundle",
      "lineageVerifierRecord is required for reserved-lineage bundles",
      'AssertArgumentDiagnostic(\n                "Kagemusha Norito archive must not be empty.",\n                "noritoBytes",',
      'AssertArgumentDiagnostic(\n                "Kagemusha Norito archive must contain a non-empty Norito payload.",\n                "noritoBytes",',
      'Assert.Equal(\n            "connect_norito_kagemusha_verify_recursive_compact_payment_token returned invalid boolean output 2.",',
      'Assert.Equal(\n            "connect_norito_kagemusha_verify_recursive_compact_payment_token failed with bridge error code -311.",',
      "CompactTokenProverRejectsMalformedInputsBeforeLoadingNativeBridge",
      "CompactTokenProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
      "RecursiveAggregationProverRejectsMalformedInputsBeforeLoadingNativeBridge",
      "RecursiveAggregationProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
      "RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge",
      "RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
      "RecursiveSpendCompactProjectionRejectsInvalidBundleBeforeLoadingNativeBridge",
      'AssertArgumentDiagnostic(\n            "Record bundle archive must be a valid Norito archive.",\n            "recordBundleArchive",',
      'AssertArgumentDiagnostic(\n            "Record bundle archive must contain a non-empty Norito payload.",\n            "recordBundleArchive",',
      "Recursive spend bundle archive must be a valid Norito archive",
      'AssertArgumentDiagnostic(\n            "Pallas open-envelopes archive must be a valid Norito archive.",\n            "pallasOpenEnvelopesArchive",',
      'AssertArgumentDiagnostic(\n            "Pallas open-envelopes archive must contain a non-empty Norito payload.",\n            "pallasOpenEnvelopesArchive",',
      "KagemushaNoritoFrameWithPayload",
      "AssertRejectsMalformedEverywhere",
      "AssertRejectsMalformedEverywhere(compressed, validArchive, validRecordBundle)",
      "AssertRejectsMalformedEverywhere(unsupportedFlags, validArchive, validRecordBundle)",
      "AssertRejectsMalformedEverywhere(invalidFieldBitset, validArchive, validRecordBundle)",
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

test("Kagemusha Swift V2 archives stay canonical and ABI-18 bounded", () => {
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"),
    [
      "public static let requiredNativeBridgeAbiVersion: UInt32 = 18",
      "public static let artifactMaximumFileBytes = 256 * 1024 * 1024",
      "static func requireArchive(_ archive: Data, schema: String, field: String) throws",
      "archive.count <= artifactMaximumFileBytes",
      "frame.header.schema == noritoSchemaHash(forTypeName: schema)",
      "frame.header.compression == .none",
      "frame.header.flags == NoritoHeader.compactLen",
      "frame.paddingLength == 0",
      "!frame.payload.isEmpty",
      "throw KagemushaRecursiveSpendError.invalidArchive(field)",
    ],
    "Swift V2 canonical archive guards",
  );
  assertContainsAll(
    source("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2Codecs.swift"),
    [
      "public enum KagemushaRecursiveSpendCodecs",
      "let frame = noritoDecodeFrame(pallasOpenEnvelopes)",
      "decoded.paddingLength == 0",
      "throw KagemushaRecursiveSpendError.invalidArchive(field)",
      "try reader.finish(",
    ],
    "Swift V2 canonical codecs",
  );
  assertContainsAll(
    source("IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendV2Tests.swift"),
    [
      "testABI18InventoryRequiresExplicitFailClosedCapabilities",
      "testNativeCapabilitiesRequireExactABI18ContractAndGateSet",
      "testTopUpFinalityOpaqueTypesPinExactNoritoSchemasAndCopyBytes",
      "testFlatInitRequestRoundTripsWithMandatoryFinalityProof",
      '.invalidArchive("topUpFinalityProof")',
      "KagemushaRecursiveSpend.requiredNativeBridgeAbiVersion, 18",
      "KagemushaRecursiveSpend.releaseMaximumProofBytes, 4_096",
    ],
    "Swift V2 canonical archive and ABI-18 tests",
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
      "string? publicAmount",
      "string? currentNoteAmount",
      "bool hasChangeOutput",
      "KagemushaRecursiveSpendNative.Redeem(\n            redeemRequestArchive,\n            publicAmount,\n            currentNoteAmount,\n            hasChangeOutput)",
      "KagemushaRecursiveSpendNative.Redeem(\n            redeemRequestArchive,\n            proofCircuitId,\n            hopCount,\n            hasLineageWitness,\n            hasLineageVerifierRecord)",
      "KagemushaRecursiveSpendNative.Redeem(\n            redeemRequestArchive,\n            proofCircuitId,\n            hopCount,\n            hasLineageWitness,\n            hasLineageVerifierRecord,\n            lineageVerifierRecordCount)",
      "KagemushaRecursiveSpendNative.Redeem(\n            redeemRequestArchive,\n            proofCircuitId,\n            hopCount,\n            hasLineageWitness,\n            hasLineageVerifierRecord,\n            publicAmount,\n            currentNoteAmount,\n            hasChangeOutput)",
      "KagemushaRecursiveSpendNative.Redeem(\n            redeemRequestArchive,\n            proofCircuitId,\n            hopCount,\n            hasLineageWitness,\n            hasLineageVerifierRecord,\n            lineageVerifierRecordCount,\n            publicAmount,\n            currentNoteAmount,\n            hasChangeOutput)",
      "KagemushaRecursiveSpendRedeemInstructionArchive",
    ],
    "C# Kagemusha recursive redeem transaction builder",
  );
  assertContainsAll(
    source("csharp/tests/Hyperledger.Iroha.Sdk.Tests/TransactionBuilderTests.cs"),
    [
      "AddInstructionAcceptsKagemushaInstructionArchiveFactories",
      "KagemushaRecursiveRedeemMetadataOverloadRejectsInvalidChangeOutputBeforeNativeBridge",
      "KagemushaRecursiveRedeemMetadataOverloadRejectsInvalidLineageBeforeNativeBridge",
      "KagemushaRecursiveRedeemMetadataOverloadRejectsLineageAndAmountDriftBeforeNativeBridge",
      "KagemushaRecursiveRedeemMetadataOverloadsAllowValidRelationshipsBeforeNativeRequestValidation",
      "BuildSignedEmbedsKagemushaInstructionArchiveWithoutReframing",
      "KagemushaInstructionArchiveRejectsMalformedWrongTypeAndMismatchedType",
      "KagemushaInstructionArchiveAcceptsNativeAbi7RedeemInstructionFixture",
      "Kagemusha instruction archive must not be empty.",
      "Kagemusha instruction archive must be a valid Norito instruction archive.",
      "Kagemusha instruction archive schema must match RedeemKagemushaRecursive.",
      "Kagemusha instruction archive must contain a non-empty Norito payload.",
      "KagemushaArchive(KagemushaInstructionType.RedeemRecursive, Array.Empty<byte>())",
      '"instructionArchive"',
      "KagemushaInstructionType.RedeemRecursive",
      "KagemushaInstructionType.Transfer",
      "new KagemushaRecursiveSpendRedeemInstructionArchive(redeemArchive)",
      "malformedRequestArchive",
      "Assert.Empty(builder.Instructions)",
      "AssertArgumentDiagnostic(",
      "changeOutput is required when publicAmount is less than current note amount",
      "publicAmount must be less than current note amount when changeOutput is present",
      "publicAmount must not exceed current note amount",
      "changeOutput must be exactly 32 bytes",
      "changeOutput must be non-zero",
      '$"{expectedParamName} must be a decimal integer"',
      "lineageWitness is required for this bundle",
      "lineageVerifierRecord is required for reserved-lineage bundles",
      "lineageVerifierRecords count must be non-negative",
      "Request archive must be a valid Norito archive.",
      '"hasChangeOutput"',
      '"hasLineageWitness"',
      '"lineageVerifierRecordCount"',
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
    source("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/TransactionPayload.kt"),
    [
      "require(chainId.trim().isNotEmpty())",
      'require(chainId.trim() == chainId) { "chainId must not contain surrounding whitespace" }',
      'requireCanonicalI105Address(authority, "authority")',
    ],
    "Kotlin transaction payload exact identifier validation",
  );
  assertContainsAll(
    source("java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/TransactionPayload.java"),
    [
      'this.chainId = normalizeExact(chainId, "chainId")',
      "private static String normalizeExact(final String value, final String field)",
      'field + " must not contain surrounding whitespace"',
      'AccountIdLiteral.requireCanonicalI105Address(authority, "authority")',
    ],
    "Android Java transaction payload exact identifier validation",
  );
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
      'chainId = requireNonBlankUnpadded(chainId, "chainId")',
      'authority = requireNonBlankUnpadded(authority, "authority")',
      'val exactChainId = requireNonBlankUnpadded(chainId, "chainId")',
      'val exactAuthority = requireNonBlankUnpadded(authority, "authority")',
      'require(value.trim() == value) { "$field must not contain surrounding whitespace" }',
      "val archive = instructionArchive.copyOf()",
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
      'setChainId(requireNonBlankUnpadded(chainId, "chainId"))',
      'setAuthority(requireNonBlankUnpadded(authority, "authority"))',
      'final String exactChainId = requireNonBlankUnpadded(chainId, "chainId")',
      'final String exactAuthority = requireNonBlankUnpadded(authority, "authority")',
      'field + " must not contain surrounding whitespace"',
      "final byte[] archive = instructionArchive.clone();",
      "InstructionBox.fromWirePayload(instructionType.wireName(), archive)",
      "Collections.singletonList(instructionBox(instructionType, instructionArchive))",
    ],
    "Android Java Kagemusha instruction archive transaction helper",
  );
  assertContainsAll(
    source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaInstructionArchivesTest.kt"),
    [
      "instructionBox preserves redeem archive bytes and wire name",
      "transactionPayload wraps a single transfer archive instruction",
      "recursiveRedeemTransactionPayload preserves redeem archive bytes after caller mutation",
      "transactionPayload rejects padded ids before archive validation or native redeem",
      "instructionBox accepts native ABI 7 redeem instruction fixture",
      "instructionBox rejects malformed wrong schema empty and tampered archives",
      "KagemushaInstructionType.REDEEM_RECURSIVE",
      "KagemushaInstructionType.TRANSFER",
      "assertContentEquals(kagemushaArchive(KagemushaInstructionType.REDEEM_RECURSIVE), payload.payloadBytes)",
      "val expectedArchive = archive.copyOf()",
      '"fixture" to "string metadata"',
      'payload.metadata["fixture"]',
      "archive[0] = 0x7f.toByte()",
      "assertContentEquals(expectedArchive, wire.payloadBytes)",
      "chainId must not contain surrounding whitespace",
      "authority must not contain surrounding whitespace",
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
  assert.doesNotMatch(
    source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaInstructionArchivesTest.kt"),
    /"leg[\s\S]*?acy" to "string metadata"|payload\.metadata\["leg[\s\S]*?acy"\]/u,
    "Kotlin Kagemusha instruction archive metadata fixture must not use old-path key names",
  );
  assertContainsAll(
    source("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/TransactionPayloadTest.kt"),
    [
      "padded chainId throws before payload can be signed",
      "authority must be exact canonical I105 before payload can be signed",
      "chainId must not contain surrounding whitespace",
      "authority must not contain surrounding whitespace",
      "authority must use canonical I105 encoded account without @domain",
    ],
    "Kotlin transaction payload exact identifier tests",
  );
  assertContainsAll(
    source("java/iroha_android/src/test/java/org/hyperledger/iroha/android/tx/TransactionBuilderTests.java"),
    [
      "transactionPayloadRejectsPaddedIdsBeforeSigning",
      "kagemushaInstructionArchivesBuildPayloads",
      "kagemushaInstructionArchivesAcceptAbi7Fixtures",
      "kagemushaInstructionArchivesRejectPaddedIdsBeforeArchiveOrNativeRedeem",
      "kagemushaInstructionArchivesRejectAdversarialInputs",
      "KagemushaInstructionArchives.InstructionType.REDEEM_RECURSIVE",
      "KagemushaInstructionArchives.InstructionType.TRANSFER",
      "KagemushaInstructionArchives.recursiveRedeemInstructionBox(archive)",
      "KagemushaInstructionArchives.transactionPayload(",
      "KagemushaInstructionArchives.recursiveRedeemInstructionBoxFromRequest(new byte[0])",
      "KagemushaInstructionArchives.recursiveRedeemTransactionPayloadFromRequest(",
      "Arrays.equals(transferArchive, transferWire.payloadBytes())",
      "final byte[] expectedTransferArchive",
      "transferArchive[0] = (byte) 0x7F",
      "Arrays.equals(expectedTransferArchive, transferWire.payloadBytes())",
      "assertIllegalArgumentMessage",
      "chainId must not contain surrounding whitespace",
      "authority must not contain surrounding whitespace",
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
        "buildKagemushaRecursiveTopUpTransaction",
        "KagemushaInstructionArchive",
        "KagemushaTransfer",
        "RedeemKagemushaRecursive",
      "instruction_type",
      'typeof type !== "string"',
      "normalizeTransactionAssetDefinitionId",
      "authority must not contain surrounding whitespace",
      "assetDefinition.assetDefinitionId",
      "normalizeExactMetadataString",
      "verifyingKey.id.backend",
      "verifyingKey.record.circuit_id",
      "must not contain surrounding whitespace",
      "kagemushaRecursiveSpendRedeem",
      "kagemushaRecursiveRedeem.redeemRequestArchive",
      "kagemushaRecursiveSpendTopUp",
      "kagemushaRecursiveTopUp.topUpRequestArchive",
      "Buffer.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength))",
      "Buffer.from(new Uint8Array(value))",
    ],
      `${relative} Kagemusha instruction transaction builder`,
    );
    assert.match(
      transactionSource,
      /buildKagemushaRecursiveRedeemTransaction[\s\S]*?kagemushaRecursiveSpendRedeem[\s\S]*?buildKagemushaInstructionTransaction/u,
      `${relative} must derive the redeem instruction before signing`,
    );
    assert.match(
      transactionSource,
      /buildKagemushaRecursiveTopUpTransaction[\s\S]*?kagemushaRecursiveSpendTopUp[\s\S]*?buildKagemushaInstructionTransaction/u,
      `${relative} must derive the top-up instruction before signing`,
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
        "buildKagemushaRecursiveTopUpTransaction",
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
      '"TopUpKagemushaRecursive"',
      "KagemushaInstructionArchiveInput",
      "KagemushaInstructionTransactionInput",
      "KagemushaRecursiveRedeemTransactionBaseInput",
      "KagemushaRecursiveRedeemArchiveInput",
      "KagemushaRecursiveRedeemTransactionInput",
      "KagemushaRecursiveTopUpTransactionBaseInput",
      "KagemushaRecursiveTopUpArchiveInput",
      "KagemushaRecursiveTopUpTransactionInput",
      "buildKagemushaInstructionArchiveInstruction",
      "buildKagemushaInstructionTransaction",
      "buildKagemushaRecursiveRedeemTransaction",
      "buildKagemushaRecursiveTopUpTransaction",
      "topUpRequestArchive: BinaryLike;",
      "top_up_request_archive: BinaryLike;",
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
      "buildKagemushaRecursiveTopUpTransaction derives top-up instruction before signing",
      "buildKagemushaRecursiveRedeemTransaction derives instruction before signing",
      "transaction builders reject padded authority and asset definition IDs before native dispatch",
      "authority must not contain surrounding whitespace",
      "assetDefinition\\.assetDefinitionId must not contain surrounding whitespace",
      "proof builders reject padded inline verifier-key metadata",
      "buildPrivateKaigiFeeSpend",
      "privateKaigiFeeSpend\\.verifyingKey\\.id\\.backend must not contain surrounding whitespace",
      "privateKaigiFeeSpend\\.verifyingKey\\.record\\.circuit_id must not contain surrounding whitespace",
      "instruction_type: \"KagemushaTransfer\"",
      "kagemushaRecursiveTopUp\\.topUpRequestArchive must be a Buffer or ArrayBuffer view",
      "top-up native rejected",
      "redeemRequestArchive must be a Buffer or ArrayBuffer view",
      "redeem native rejected",
      "buildKagemusha transaction helpers copy mutable buffers before native calls",
      "mutableTransferArchive",
      "mutableRedeemInstructionArchive",
      "mutableRedeemRequestArchive",
      "mutablePrivateKey",
      "fill(0xa5)",
    ],
    "JavaScript Kagemusha instruction transaction builder tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package dist Kagemusha transaction helpers copy mutable buffers before native calls",
      "package dist Kagemusha transaction helpers reject padded authority before native dispatch",
      "authority must not contain surrounding whitespace",
      "PACKAGE_DIST_KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES",
      "packageDistKagemushaInstructionArchive",
      "buildKagemushaInstructionArchiveInstruction",
      "buildKagemushaInstructionTransaction",
      "buildKagemushaRecursiveRedeemTransaction",
      "mutableTransferArchive",
      "mutableRedeemInstructionArchive",
      "mutableRedeemRequestArchive",
      "mutablePrivateKey",
      "fill(0xa5)",
    ],
    "JavaScript package dist Kagemusha instruction transaction builder tests",
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
      '_kagemusha_require_non_blank_unpadded(chain_id, "chain_id")',
      '_kagemusha_require_non_blank_unpadded(authority, "authority")',
      '_norito_archive_bytes_named(instruction_archive, "instruction_archive")',
      'getattr(Instruction, "kagemusha_instruction_archive", None)',
      'getattr(Instruction, "kagemusha_recursive_redeem", None)',
      "instruction = kagemusha_recursive_redeem_instruction(redeem_request_archive)",
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
    source("python/iroha_python/src/iroha_python/tx.py"),
    [
      "def _require_exact_non_empty_string(value: Any, context: str) -> str:",
      'chain_id=_require_exact_non_empty_string(config.chain_id, "chain_id")',
      'authority=_require_exact_non_empty_string(config.authority, "authority")',
      '_require_exact_non_empty_string(chain_id, "chain_id")',
      "if chain_id is not None",
      '_require_exact_non_empty_string(authority, "authority")',
      "if authority is not None",
    ],
    "Python TransactionDraft exact identifier config and override guards",
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
      "fn require_non_blank_unpadded(value: &str, field: &str) -> PyResult<()>",
      'require_non_blank_unpadded(chain_id, "chain_id")?',
      'require_non_blank_unpadded(authority, "authority")?',
      "fn transaction_builder_rejects_padded_chain_id_and_authority()",
      "ValueError: chain_id must not contain surrounding whitespace",
      "ValueError: authority must not contain surrounding whitespace",
    ],
    "Python PyO3 Kagemusha instruction archive decoder and TransactionBuilder exactness",
  );
  assertContainsAll(
    source("python/iroha_python/src/iroha_python/client.py"),
    [
      "def _transaction_draft(",
      'effective_chain_id = _require_exact_non_empty_string(chain_id, "chain_id")',
      'effective_authority = self._native_transaction_account_id(',
      '_require_exact_non_empty_string(authority, "authority")',
    ],
    "Python Torii transaction draft exactness",
  );
  assertContainsAll(
    source("python/iroha_python/tests/kagemusha_test.py"),
    [
      "test_kagemusha_instruction_archive_transaction_helpers_wrap_redeem_archive",
      "test_kagemusha_recursive_redeem_transaction_helper_derives_instruction_before_signing",
      "test_kagemusha_instruction_transaction_helpers_copy_mutable_archives_before_building",
      "test_kagemusha_instruction_transaction_helpers_reject_padded_chain_and_authority_before_signing",
      "test_python_transaction_builder_rejects_padded_chain_and_authority_before_signing",
      "test_python_transaction_draft_rejects_padded_config_and_sign_overrides_before_signing",
      "test_kagemusha_instruction_archive_transaction_helpers_reject_adversarial_inputs",
      'iroha_python.TransactionBuilder(" chain", authority)',
      "iroha_python.TransactionDraft(",
      'iroha_python.TransactionConfig(chain_id=" chain", authority=authority)',
      'iroha_python.TransactionConfig(chain_id="", authority=authority)',
      'iroha_python.TransactionConfig(chain_id="chain", authority="")',
      'draft.sign(keypair.private_key, chain_id=" chain")',
      'draft.sign(keypair.private_key, chain_id="")',
      'draft.sign(keypair.private_key, authority=f"{authority} ")',
      'draft.sign(keypair.private_key, authority="")',
      "chain_id must be non-empty",
      "authority must be non-empty",
      "iroha_python.build_signed_transaction(",
      "redeem_instruction_archive = bytearray(",
      "redeem_request_archive = bytearray(",
      "private_key = bytearray(keypair.private_key)",
      "memoryview(redeem_instruction_archive)",
      "memoryview(redeem_request_archive)",
      "memoryview(private_key)",
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
      "chain_id must not contain surrounding whitespace",
      "authority must not contain surrounding whitespace",
      "bad_request_flags[39] = 0x20",
      "redeem_request_archive must be a valid Norito archive",
      "draft.kagemusha_recursive_redeem(request_archive)",
    ],
    "Python Kagemusha instruction transaction tests",
  );
  assertContainsAll(
    source("python/iroha_python/tests/client_ledger_helpers_test.py"),
    [
      "test_transaction_draft_rejects_padded_chain_and_authority_before_signing",
      'client._transaction_draft(chain_id=" chain", authority="authority@is")',
      'client._transaction_draft(chain_id="chain", authority=" authority@is ")',
      "chain_id must not contain surrounding whitespace",
      "authority must not contain surrounding whitespace",
    ],
    "Python Torii transaction draft exact identifier tests",
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
      "Proof payloads below the ABI-7 compact floor return ERR_KAGEMUSHA_PROVE.",
      "Preverified tokens with cryptographically invalid proof bodies return success",
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
  assert.match(
    dts,
    /export type KagemushaRecursiveSpendAppendRequestInput =\s*KagemushaRecursiveSpendAppendRequestBaseInput &\s*\(\s*\|\s*\{\s*readonly outputProofCircuitId: string;\s*readonly output_proof_circuit_id\?: never;\s*\}\s*\|\s*\{\s*readonly outputProofCircuitId\?: never;\s*readonly output_proof_circuit_id: string;\s*\}\s*\);/u,
    "JavaScript TypeScript recursive append request declaration must require exactly one output selector alias",
  );
  assert.doesNotMatch(
    dts,
    /readonly output(?:ProofCircuitId|_proof_circuit_id)\?: string \| null/u,
    "JavaScript TypeScript recursive append output selector must not be optional or nullable",
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
      "new Uint8Array(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1)",
      "[undefined, \"must be a Buffer, string, or ArrayBuffer view\"]",
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
      "lineageDigestV1",
      "aggregationTranscriptDigestV1",
      "fixedWindowTableScheduleDigestV1",
      "fixedWindowSharedTableManifestDigestV1",
      "fixedWindowTableBaseDigestV1",
      "verifierWitnessBatchDigestV1",
      "recursiveProofChainDigestV1",
      "proofChainDigestV1",
      "transitionProfileBindingDigestV1",
      "appendOpeningPreflightDigestV1",
      "appendBoundaryDigestV1",
      "recursiveVerifierScalarProjectionDigestV1",
      "previousAccumulatorDigestV1",
      "resultingAccumulatorDigestV1",
      "accumulatorDigestV1",
      "ProofChainState",
      "AppendAccumulatorState",
      "recursiveAccumulatorV1",
      "AccumulatorStateBytes",
      "inputTerminalAccumulator",
      "staleWalletRecursiveProofChain",
      "nativeAppendAccumulatorState",
      "publicProofChainBytes",
    ],
    "JavaScript package recursive compact declaration tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package dist Kagemusha recursive spend compact projection helpers dispatch owned archives",
      "isKagemushaRecursiveSpendCompactPaymentTokenProjectionNativeAvailable()",
      "isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierNativeAvailable()",
      "kagemushaRecursiveSpendCompactPaymentTokenFromBundle(",
      "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection(",
      "0xffff_ffff_ffff_ffffn",
      "0x1_0000_0000_0000_0000n",
      "Number.MAX_SAFE_INTEGER + 1",
      "/blockHeight must be a number or bigint/",
      "/blockHeight must be an integer/",
      "/blockHeight must be non-negative/",
      "/blockHeight number must be a safe integer/",
      "/blockHeight must fit in u64/",
      "assert.notStrictEqual(calls[0][1], bundleArchive)",
      "assert.notStrictEqual(call[1], compactTokenArchive)",
      "assert.notStrictEqual(call[2], verifierRecordArchive)",
      "assert.deepEqual(projection, expectedProjectionOutput)",
    ],
    "JavaScript package recursive spend compact projection dispatch tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package dist Kagemusha recursive spend compact projection helpers fail closed on invalid archives",
      "const invalidArchives = [",
      "[Buffer.alloc(0), \"must not be empty\"]",
      "[Buffer.from([0x01]), \"must be a valid Norito archive\"]",
      "[privacyNoritoFrame(0x85), \"must contain a non-empty Norito payload\"]",
      "bundleArchive ${expectedMessage}",
      "compactTokenArchive ${expectedMessage}",
      "verifierRecordArchive ${expectedMessage}",
      "invalidArchive,\n            validArchive,\n            9,",
      "validArchive,\n            invalidArchive,\n            9,",
      "assert.equal(nativeDispatches, 0)",
      "/returned invalid Norito archive/",
      "/kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection returned a non-boolean result/",
    ],
    "JavaScript package recursive spend compact projection fail-closed tests",
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
      "verifier_record_archive {expected_message}",
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
      'AssertArgumentDiagnostic(\n            "Compact token archive must not be empty.",\n            "compactTokenArchive",',
      'AssertArgumentDiagnostic(\n            "Compact token archive must be a valid Norito archive.",\n            "compactTokenArchive",',
      'AssertArgumentDiagnostic(\n            "Compact token archive must contain a non-empty Norito payload.",\n            "compactTokenArchive",',
      'AssertArgumentDiagnostic(\n            "Recursive compact verifier keys archive must not be empty.",\n            "recursiveCompactVerifierKeysArchive",',
      'AssertArgumentDiagnostic(\n            "Recursive compact verifier keys archive must be a valid Norito archive.",\n            "recursiveCompactVerifierKeysArchive",',
      'AssertArgumentDiagnostic(\n            "Recursive compact verifier keys archive must contain a non-empty Norito payload.",\n            "recursiveCompactVerifierKeysArchive",',
      'AssertArgumentDiagnostic(\n            "Verifier record archive must be a valid Norito archive.",\n            "verifierRecordArchive",',
      'AssertArgumentDiagnostic(\n            "Verifier record archive must contain a non-empty Norito payload.",\n            "verifierRecordArchive",',
      "VerifyRecursiveSpendCompactPaymentTokenProjection",
      "PallasOpenEnvelopeBuildersRejectMalformedInputsBeforeLoadingNativeBridge",
      "PallasOpenEnvelopeBuildersRejectOversizedInputsBeforeLoadingNativeBridge",
      "PallasOpenEnvelopeBuildersRejectEmptyPayloadInputsBeforeLoadingNativeBridge",
      "PallasOpenEnvelopeBuilderReadBridgeOutputRejectsMalformedNoritoSuccessOutput",
      "PallasOpenEnvelopeBuilderReadBridgeOutputRejectsEmptyPayloadNoritoSuccessOutput",
      "BuildPallasOpenEnvelopesArchive",
      "BuildPreviousProofOpenEnvelopesArchive",
      'AssertArgumentDiagnostic(\n            "Previous recursive proof bundle archive must be a valid Norito archive.",\n            "previousBundleArchive",',
      'AssertArgumentDiagnostic(\n            "Previous recursive proof bundle archive must contain a non-empty Norito payload.",\n            "previousBundleArchive",',
      "connect_norito_kagemusha_build_pallas_open_envelopes_archive returned invalid Norito archive",
      "connect_norito_kagemusha_build_previous_proof_open_envelopes_archive returned empty Norito payload",
      "RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge",
      "RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge",
      'AssertArgumentDiagnostic(\n            "Recursive compact key artifacts archive must be a valid Norito archive.",\n            "recursiveCompactKeyArtifactsArchive",',
      'AssertArgumentDiagnostic(\n            "Recursive compact key artifacts archive must contain a non-empty Norito payload.",\n            "recursiveCompactKeyArtifactsArchive",',
      "RecursiveSpendCompactProjectionRejectsInvalidBundleBeforeLoadingNativeBridge",
      "RecursiveSpendCompactProjectionVerifierRejectsInvalidInputsBeforeLoadingNativeBridge",
      'AssertArgumentDiagnostic(\n            "Recursive spend bundle archive must be a valid Norito archive.",\n            "bundleArchive",',
      'AssertArgumentDiagnostic(\n            "Recursive spend bundle archive must contain a non-empty Norito payload.",\n            "bundleArchive",',
      "RecursiveSpendNativeReadBridgeOutputRejectsMalformedNoritoSuccessOutput",
      "RecursiveSpendNativeReadBridgeOutputRejectsEmptyPayloadNoritoSuccessOutput",
      "RecursiveSpendNativeReadBridgeOutputReturnsValidNoritoSuccessOutput",
      'Assert.Equal(\n            "connect_norito_kagemusha_recursive_spend_redeem failed with bridge error code -311.",',
      'Assert.Equal(\n            "connect_norito_kagemusha_recursive_spend_redeem returned empty output.",',
      'Assert.Equal(\n                "connect_norito_kagemusha_recursive_spend_redeem returned invalid Norito archive.",',
      'Assert.Equal(\n            "connect_norito_kagemusha_build_pallas_open_envelopes_archive returned invalid Norito archive.",',
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
        "Kagemusha compact payment-token prover requires native bridge ABI 18",
        "Kagemusha recursive aggregation proof-bundle prover requires native bridge ABI 18",
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
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package dist Kagemusha record-backed and Pallas builders dispatch owned archives",
      "isKagemushaCompactPaymentTokenNativeAvailable()",
      "isKagemushaRecursiveAggregationProofBundleNativeAvailable()",
      "isKagemushaPallasOpenEnvelopeBuilderNativeAvailable()",
      "kagemushaProveVerifiedCompactPaymentTokenWithRecords(",
      "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(",
      "kagemushaBuildPallasOpenEnvelopesArchive(",
      "kagemushaBuildPreviousProofOpenEnvelopesArchive(",
      "assert.notStrictEqual(calls[0][1], recordBundle)",
      "assert.notStrictEqual(calls[1][2], pallasOpenEnvelopes)",
      "assert.notStrictEqual(calls[3][1], previousBundle)",
      "assert.deepEqual(result, expectedOutputs.get(methodName), methodName)",
    ],
    "JavaScript package record-backed Kagemusha and Pallas builder dispatch tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package dist Kagemusha record-backed and Pallas builders fail closed on invalid archives",
      "const invalidArchives = [",
      "[Buffer.alloc(0), \"must not be empty\"]",
      "[Buffer.from([0x01]), \"must be a valid Norito archive\"]",
      "[privacyNoritoFrame(0x98), \"must contain a non-empty Norito payload\"]",
      "[Buffer.alloc(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f), \"must not exceed\"]",
      "[undefined, \"must be a Buffer, string, or ArrayBuffer view\"]",
      'const invalidArchives = [\n    [Buffer.alloc(0), "must not be empty"],\n    [Buffer.from([0x01]), "must be a valid Norito archive"],\n    [privacyNoritoFrame(0x98), "must contain a non-empty Norito payload"],\n    [Buffer.alloc(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f), "must not exceed"],\n    [undefined, "must be a Buffer, string, or ArrayBuffer view"],\n  ];',
      "recordBundleArchive ${expectedMessage}",
      "pallasOpenEnvelopesArchive ${expectedMessage}",
      "previousBundleArchive ${expectedMessage}",
      "assert.equal(nativeDispatches, 0)",
      "native kagemushaProveVerifiedCompactPaymentTokenWithRecords returned invalid Norito archive",
      "native kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes returned empty Norito payload",
      "native kagemushaBuildPallasOpenEnvelopesArchive returned invalid Norito archive",
      "native kagemushaBuildPreviousProofOpenEnvelopesArchive returned empty Norito payload",
    ],
    "JavaScript package record-backed Kagemusha and Pallas builder fail-closed tests",
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

test("recursive Kagemusha ABI-18 availability probes require transition-profile, boundary, and lineage-witness helpers", () => {
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
        "18.5",
        "-1",
        "isKagemushaRecursiveCompactPaymentTokenNativeAvailable(), false",
      ],
      `${relative} Kagemusha ABI probe tests`,
    );
  }
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package dist Kagemusha recursive spend helpers dispatch owned archive copies and return Buffers",
      "assert.equal(isKagemushaRecursiveSpendNativeAvailable(), true)",
      "assert.ok(Buffer.isBuffer(result), methodName)",
      "assert.notStrictEqual(result, nativeOutputs.get(methodName), methodName)",
      "assert.notStrictEqual(call[argIndex + 1], args[argIndex], methodName)",
      "assert.deepEqual(call[argIndex + 1], expectedInputs[index][argIndex], methodName)",
      "kagemushaRecursiveSpendLineageWitnessAppendResult(",
    ],
    "JavaScript package dist recursive spend positive dispatch and copy tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package dist Kagemusha recursive spend availability rejects partial ABI-18 surfaces",
      "const requiredMethods = [",
      '"kagemushaRecursiveSpendTransitionProfileInit"',
      '"kagemushaRecursiveSpendTransitionProfileAppend"',
      '"kagemushaRecursiveSpendLineageAppendBoundary"',
      '"kagemushaRecursiveSpendLineageWitnessFromInitResult"',
      '"kagemushaRecursiveSpendLineageWitnessAppendResult"',
      "delete binding[missingMethod]",
      "preferredKagemushaOfflineSpendMode()",
    ],
    "JavaScript package dist recursive spend partial ABI-18 availability tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package dist Kagemusha recursive spend availability rejects broken and permissive native probes",
      "throw new Error(\"bridge denied\")",
      "const acceptedMethods = [",
      '"kagemushaRecursiveSpendTransitionProfileInit"',
      '"kagemushaRecursiveSpendTransitionProfileAppend"',
      '"kagemushaRecursiveSpendLineageAppendBoundary"',
      '"kagemushaRecursiveSpendLineageWitnessFromInitResult"',
      '"kagemushaRecursiveSpendLineageWitnessAppendResult"',
      "return Uint8Array.from([0xff])",
      "preferredKagemushaOfflineSpendMode()",
      "Kagemusha recursive spend helper 'kagemushaRecursiveSpendVerify' is unavailable",
    ],
    "JavaScript package dist recursive spend broken/permissive probe tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package dist Kagemusha recursive spend helpers reject unsafe native outputs",
      "const invalidOutputs = [",
      "[Buffer.alloc(0), /returned empty output/]",
      "[null, /returned no output/]",
      '["not-bytes", /returned text instead of Norito bytes/]',
      "Buffer.alloc(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f)",
      "[Buffer.from([0x01]), /returned invalid Norito archive/]",
      "[privacyNoritoFrame(0x36), /returned empty Norito payload/]",
      '"kagemushaRecursiveSpendTransitionProfileInit"',
      '"kagemushaRecursiveSpendTransitionProfileAppend"',
      '"kagemushaRecursiveSpendLineageAppendBoundary"',
      '"kagemushaRecursiveSpendLineageWitnessFromInitResult"',
      '"kagemushaRecursiveSpendLineageWitnessAppendResult"',
      "completeBinding(methodName, output)",
    ],
    "JavaScript package dist recursive spend unsafe native output tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package dist Kagemusha recursive spend helpers reject invalid request archives before native dispatch",
      "const invalidArchives = [",
      "[Buffer.alloc(0), \"must not be empty\"]",
      "new Uint8Array(KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1)",
      "[Buffer.from([0x01]), \"must be a valid Norito archive\"]",
      "[privacyNoritoFrame(0x35), \"must contain a non-empty Norito payload\"]",
      "[undefined, \"must be a Buffer, string, or ArrayBuffer view\"]",
      '"previousWitnessArchive"',
      '"profileArchive"',
      "assert.equal(nativeDispatches, 0)",
    ],
    "JavaScript package dist recursive spend invalid request archive tests",
  );
  assertContainsAll(
    source("javascript/iroha_js/test/package_dist.test.js"),
    [
      "package dist Kagemusha recursive spend helpers propagate native semantic rejections",
      '"redeem-over-cap"',
      '"verify-forged-lineage"',
      '"redeem-forged-lineage"',
      '"transition-profile-append-forged-opening"',
      "/bundle\\.accumulator\\.hop_count/",
      "/lineage_verifier_record\\.commitment/",
      "/hop domain metadata mismatch/",
      "assert.notStrictEqual(archive, requests.get(label), label)",
      "assert.deepEqual(archive, expectedRequests.get(label), label)",
    ],
    "JavaScript package dist recursive spend native semantic rejection tests",
  );

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
      "test_recursive_kagemusha_availability_requires_exact_bridge_abi_18",
      '"18"',
      "18.5",
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

test("recursive Kagemusha witnessless Reserved-lineage policy stays fail-closed in public docs", () => {
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
    /TRANSITION_CIRCUIT_WIRED_V1[^.\n]*(?:true|`true`)/iu,
    /witnessless\s+Reserved-lineage[^.\n]*(?:enabled|available|admitted)/iu,
    /canAppendWitnesslessLineage[^.\n]*returns\s+`?true`?/iu,
  ];

  const rustDataModel = source("crates/iroha_data_model/src/offline/mod.rs");
  assert.match(
    rustDataModel,
    /KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1:\s*u32\s*=\s*64\s*;/u,
    "Rust data model must expose the 64-hop witnessless Reserved-lineage cap",
  );
  assert.match(
    rustDataModel,
    /KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1:\s*bool\s*=\s*false\s*;/u,
    "Rust data model must keep the Reserved-lineage transition circuit fail-closed",
  );

  for (const relativePath of docs) {
    const text = source(relativePath);
    assert.match(
      text,
      /(?:TRANSITION_CIRCUIT_WIRED_V1|TransitionCircuitWiredV1|transitionCircuitWiredV1)[^.\n]*(?:false|`false`)|witnessless\s+Reserved-lineage[^.\n]*(?:fail\s+closed|disabled|not\s+admitted)/iu,
      `${relativePath} must document the fail-closed witnessless Reserved-lineage boundary`,
    );
    for (const forbidden of forbiddenClaims) {
      assert.doesNotMatch(text, forbidden, `${relativePath} contains a stale enabled witnessless claim`);
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
  ];
  const perSdkSnippets = new Map([
    [
      "IrohaSwift/README.md",
      [
        "KagemushaInstructionTransactionRequest",
        "IrohaSDK.buildKagemushaRecursiveRedeem(...)",
        "recursive top-up/redeem derivation inside",
      ],
    ],
    [
      "kotlin/README.md",
      [
        "KagemushaInstructionArchives",
        "builds a single archived instruction transaction payload",
        "derives the redeem instruction from a native recursive redeem request",
        "recursive top-up/redeem derivation inside",
      ],
    ],
    [
      "java/iroha_android/README.md",
      [
        "KagemushaInstructionArchives",
        "builds a single archived instruction transaction payload",
        "derives the redeem instruction from a native recursive redeem request",
        "recursive redeem derivation inside",
      ],
    ],
    [
      "csharp/README.md",
      [
        "TransactionInstruction.KagemushaInstructionArchive(...)",
        "KagemushaInstructionArchiveInstruction",
        "TransactionBuilder.KagemushaInstructionArchive(...)",
        "TransactionBuilder.KagemushaRecursiveRedeem(...)",
        "recursive redeem derivation inside",
      ],
    ],
    [
      "javascript/iroha_js/README.md",
      [
        "buildKagemushaInstructionArchiveInstruction({ instructionType, instructionArchive })",
        "buildKagemushaInstructionTransaction(...)",
        "buildKagemushaRecursiveRedeemTransaction(...)",
        "recursive redeem derivation inside",
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
        "recursive redeem derivation inside",
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
  const androidCaptureHelper = source("scripts/kagemusha_android_device_lab_capture.py");
  const androidSlotHelper = source("scripts/kagemusha_android_device_lab_slot.py");
  const androidRawPuller = source("scripts/kagemusha_pull_android_device_lab_raw_slot.py");
  const dataModel = source("crates/iroha_data_model/src/offline/mod.rs");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const verifierWitnessProfile = "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-255x1";
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
    "--negative-control-abi7-compact-package-only-verifier-dispatch",
    "--negative-control-abi7-bridge-unavailable-mapping",
    "--negative-control-abi7-offline-doc-one-hop-boundary",
    "--negative-control-offline-doc-evidence-filename-exactness",
    "--negative-control-offline-doc-compact-generator-log-exactness",
    "--negative-control-offline-doc-release-bundle-output-exactness",
    "--negative-control-offline-doc-verifier-profile-exactness",
    "--negative-control-compact-key-release-tooling",
    "--negative-control-cli-proof-attachment-envelope-hash",
    "--negative-control-connect-proof-attachment-envelope-hash",
    "--negative-control-compact-key-evidence",
    "--negative-control-compact-key-evidence-path-aliases",
    "--negative-control-localnet-lifecycle-evidence",
    "--negative-control-localnet-lifecycle-evidence-path-aliases",
    "--negative-control-localnet-lifecycle-evidence-filename",
    "--negative-control-localnet-lifecycle-evidence-filename-preflight",
    "--negative-control-localnet-lifecycle-evidence-adversarial-coverage",
    "--negative-control-localnet-lifecycle-acceptance-report-path-shape",
    "--negative-control-localnet-lifecycle-acceptance-report-filename",
    "--negative-control-localnet-lifecycle-acceptance-report-filename-preflight",
    "--negative-control-localnet-lifecycle-acceptance-identity-preflight",
    "--negative-control-localnet-lifecycle-acceptance-source-path-preflight",
    "--negative-control-localnet-lifecycle-acceptance-source-path-shape-preflight",
    "--negative-control-localnet-lifecycle-acceptance-source-identity-preflight",
    "--negative-control-localnet-lifecycle-acceptance-output-corridor-before-parent-create",
    "--negative-control-localnet-lifecycle-source-recorder-root-preflight",
    "--negative-control-localnet-lifecycle-source-recorder-private-publish",
    "--negative-control-localnet-lifecycle-helper-validation-private-permissions",
    "--negative-control-localnet-lifecycle-helper-input-corridor-resolve-failure",
    "--negative-control-localnet-lifecycle-helper-output-early-preflight",
    "--negative-control-localnet-lifecycle-helper-output-corridor-resolve-failure",
    "--negative-control-localnet-lifecycle-helper-output-private-permissions",
    "--negative-control-localnet-lifecycle-helper-output-write-failure",
    "--negative-control-localnet-lifecycle-helper-validation-before-write",
    "--negative-control-localnet-lifecycle-helper-build-errors-before-validation",
    "--negative-control-localnet-lifecycle-helper-input-errors-before-build",
    "--negative-control-localnet-lifecycle-helper-output-errors-before-input",
    "--negative-control-localnet-lifecycle-helper-raw-paths-before-path-construction",
    "--negative-control-localnet-lifecycle-evidence-helper",
    "--negative-control-localnet-lifecycle-future-skew",
    "--negative-control-localnet-lifecycle-helper-scalar-preflight",
    "--negative-control-localnet-lifecycle-helper-cli-scalar-preflight",
    "--negative-control-compact-key-artifact-prefix-binding",
    "--negative-control-compact-key-artifact-size-binding",
    "--negative-control-compact-key-evidence-json-size-limit",
    "--negative-control-compact-key-readiness-artifact-open-path-binding",
    "--negative-control-lineage-placeholder-artifacts",
    "--negative-control-compact-key-placeholder-artifacts",
    "--negative-control-compact-key-generator-log-digest-binding",
    "--negative-control-compact-key-generator-log-size-limit",
    "--negative-control-compact-key-generator-log-open-path-binding",
    "--negative-control-compact-key-helper-validation-dir-create-failure",
    "--negative-control-compact-key-helper-validation-strict-json-write",
    "--negative-control-compact-key-helper-validation-temp-write-failure",
    "--negative-control-compact-key-helper-validation-temp-cleanup-after-write-failure",
    "--negative-control-compact-key-helper-validation-temp-cleanup-failure",
    "--negative-control-compact-key-helper-validation-temp-cleanup-sync-failure",
    "--negative-control-compact-key-helper-validation-temp-cleanup-identity",
    "--negative-control-compact-key-helper-direct-artifact-dir-secret-paths",
    "--negative-control-compact-key-helper-direct-artifact-dir-metadata-failure",
    "--negative-control-compact-key-helper-direct-hash-shape",
    "--negative-control-compact-key-helper-direct-hash-read-failure",
    "--negative-control-compact-key-helper-generator-log-strict-read",
    "--negative-control-compact-key-helper-generator-log-filename-preflight",
    "--negative-control-compact-key-helper-artifact-open-path-binding",
    "--negative-control-compact-key-helper-future-skew",
    "--negative-control-compact-key-helper-scalar-preflight",
    "--negative-control-compact-key-helper-cli-scalar-preflight",
    "--negative-control-compact-key-helper-output-early-preflight",
    "--negative-control-compact-key-helper-output-file-metadata-failure",
    "--negative-control-compact-key-helper-output-hardlink-metadata-failure",
    "--negative-control-compact-key-helper-output-parent-create-failure",
    "--negative-control-compact-key-helper-output-parent-sync-identity",
    "--negative-control-compact-key-helper-output-post-write-preflight",
    "--negative-control-compact-key-helper-output-corridor-resolve-failure",
    "--negative-control-compact-key-helper-output-published-cleanup-identity",
    "--negative-control-compact-key-helper-output-published-cleanup-sync-failure",
    "--negative-control-compact-key-helper-output-readback-failure",
    "--negative-control-compact-key-helper-output-readback-open-path-binding",
    "--negative-control-compact-key-helper-output-readback-verification",
    "--negative-control-compact-key-helper-output-temp-cleanup-failure",
    "--negative-control-compact-key-helper-output-temp-cleanup-sync-failure",
    "--negative-control-compact-key-helper-output-temp-cleanup-identity",
    "--negative-control-compact-key-helper-output-write-failure",
    "--negative-control-compact-key-helper-strict-json-write",
    "--negative-control-staged-runner-exit-file-path-shape",
    "--negative-control-staged-finalizer-exit-file-path-shape",
    "--negative-control-lineage-staged-elapsed-file-path-shape",
    "--negative-control-compact-key-finalizer-exit-marker",
    "--negative-control-compact-key-finalizer-timestamp-raw",
    "--negative-control-compact-key-finalizer-future-skew",
    "--negative-control-compact-key-finalizer-publish-readback",
    "--negative-control-compact-key-finalizer-publish-rollback-identity",
    "--negative-control-compact-key-finalizer-publish-rollback-cleanup-report",
    "--negative-control-compact-key-finalizer-publish-rollback-cleanup-sync-failure",
    "--negative-control-compact-key-finalizer-publish-dir-sync-identity",
    "--negative-control-compact-key-finalizer-temp-cleanup-identity",
    "--negative-control-compact-key-finalizer-temp-cleanup-report",
    "--negative-control-compact-key-finalizer-temp-cleanup-sync-failure",
    "--negative-control-compact-key-staged-runner-exit-marker",
    "--negative-control-compact-key-staged-runner-readback",
    "--negative-control-compact-key-staged-runner-parent-sync-identity",
    "--negative-control-compact-key-staged-runner-log-install-parent-sync-identity",
    "--negative-control-compact-key-staged-runner-cleanup-identity",
    "--negative-control-compact-key-staged-runner-published-cleanup-report",
    "--negative-control-compact-key-staged-runner-published-cleanup-sync-failure",
    "--negative-control-compact-key-staged-runner-replace-cleanup-sync-failure",
    "--negative-control-compact-key-staged-runner-temp-cleanup-sync-failure",
    "--negative-control-compact-key-staged-runner-child-log-file",
    "--negative-control-compact-key-staged-runner-supervisor-output-pipe",
    "--negative-control-staged-runner-heavy-job-lock",
    "--negative-control-staged-runner-rss-limit-termination",
    "--negative-control-staged-runner-residual-process-group",
    "--negative-control-staged-runner-existing-heavy-job-conflict",
    "--negative-control-staged-runner-resource-report-fields",
    "--negative-control-staged-runner-relative-repo-root-child-path",
    "--negative-control-compact-key-staged-runner-execution-log-sha256",
    "--negative-control-compact-key-staged-runner-resume-replace-conflict",
    "--negative-control-compact-key-staged-runner-resume-artifact-prefix",
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
    "--negative-control-kagemusha-readiness-cli-path-whitespace",
    "--negative-control-kagemusha-readiness-cli-path-component-whitespace",
    "--negative-control-kagemusha-readiness-repo-root-direct-secret-paths",
    "--negative-control-kagemusha-readiness-evidence-direct-secret-paths",
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
    "--negative-control-kagemusha-readiness-missing-trusted-signer-before-slot-discovery",
    "--negative-control-kagemusha-readiness-android-report-secret-redaction",
    "--negative-control-kagemusha-readiness-android-zero-binding-digest",
    "--negative-control-kagemusha-readiness-trust-root-section-preflight",
    "--negative-control-kagemusha-readiness-android-root-discovery-read-failure",
    "--negative-control-kagemusha-readiness-summary-output-aliases",
    "--negative-control-kagemusha-readiness-summary-output-cli-path-aliases",
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
    "--negative-control-kagemusha-readiness-summary-output-temp-cleanup-sync-failure",
    "--negative-control-kagemusha-readiness-summary-output-temp-cleanup-identity",
    "--negative-control-kagemusha-readiness-summary-output-published-cleanup-identity",
    "--negative-control-kagemusha-readiness-summary-output-published-cleanup-sync-failure",
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
    "--negative-control-lineage-proof-helper-scalar-preflight",
    "--negative-control-lineage-proof-helper-cli-scalar-preflight",
    "--negative-control-lineage-proof-helper-strict-json-write",
    "--negative-control-lineage-proof-helper-output-explicit-size-cap",
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
    "--negative-control-lineage-proof-helper-validation-temp-cleanup-sync-failure",
    "--negative-control-lineage-proof-helper-validation-temp-cleanup-identity",
    "--negative-control-lineage-proof-helper-proof-log-filename-preflight",
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
    "--negative-control-lineage-proof-helper-output-temp-cleanup-sync-failure",
    "--negative-control-lineage-proof-helper-output-temp-cleanup-identity",
    "--negative-control-lineage-proof-helper-output-published-cleanup-identity",
    "--negative-control-lineage-proof-helper-output-published-cleanup-sync-failure",
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
    "--negative-control-lineage-proof-finalizer-publish-rollback-cleanup-sync-failure",
    "--negative-control-lineage-proof-finalizer-publish-dir-sync-identity",
    "--negative-control-lineage-proof-finalizer-temp-cleanup-identity",
    "--negative-control-lineage-proof-finalizer-temp-cleanup-report",
    "--negative-control-lineage-proof-finalizer-temp-cleanup-sync-failure",
    "--negative-control-lineage-proof-staged-runner-exit-marker",
    "--negative-control-lineage-proof-staged-runner-readback",
    "--negative-control-lineage-proof-staged-runner-parent-sync-identity",
    "--negative-control-lineage-proof-staged-runner-log-install-parent-sync-identity",
    "--negative-control-lineage-proof-staged-runner-cleanup-identity",
    "--negative-control-lineage-proof-staged-runner-published-cleanup-report",
    "--negative-control-lineage-proof-staged-runner-published-cleanup-sync-failure",
    "--negative-control-lineage-proof-staged-runner-replace-cleanup-sync-failure",
    "--negative-control-lineage-proof-staged-runner-temp-cleanup-sync-failure",
    "--negative-control-lineage-proof-staged-runner-child-log-file",
    "--negative-control-lineage-proof-staged-runner-supervisor-output-pipe",
    "--negative-control-staged-runner-heavy-job-lock",
    "--negative-control-staged-runner-rss-limit-termination",
    "--negative-control-staged-runner-residual-process-group",
    "--negative-control-staged-runner-existing-heavy-job-conflict",
    "--negative-control-staged-runner-resource-report-fields",
    "--negative-control-lineage-proof-staged-runner-execution-log-sha256",
    "--negative-control-lineage-proof-staged-runner-resume-replace-conflict",
    "--negative-control-lineage-proof-staged-runner-resume-artifact-content",
    "--negative-control-lineage-proof-log-exact",
    "--negative-control-lineage-proof-log-size-limit",
    "--negative-control-lineage-proof-log-is-file-preflight",
    "--negative-control-lineage-proof-log-text-preflight",
    "--negative-control-lineage-proof-log-open-path-binding",
    "--negative-control-lineage-proof-evidence-filename",
    "--negative-control-lineage-proof-evidence-filename-preflight",
    "--negative-control-lineage-proof-evidence-output-parent-sync-identity",
    "--negative-control-lineage-proof-closed-schema",
    "--negative-control-lineage-proof-evidence-helper",
    "--negative-control-android-device-lab-slot-assembler-blank-identity-override",
    "--negative-control-android-device-lab-slot-assembler-blank-source-identity",
    "--negative-control-android-device-lab-slot-assembler-adb-getprop-non-disruptive",
    "--negative-control-android-device-lab-slot-assembler-adb-getprop-timeout",
    "--negative-control-android-device-lab-slot-assembler-override-source-identity-binding",
    "--negative-control-android-device-lab-slot-assembler-source-identity-conflict",
    "--negative-control-compact-key-finalizer-execution-elapsed-binding",
    "--negative-control-compact-key-finalizer-execution-log-sha256",
    "--negative-control-compact-key-finalizer-private-permissions",
    "--negative-control-staged-finalizer-rss-terminated-report",
    "--negative-control-compact-key-generator-log-binding",
    "--negative-control-compact-key-helper-output-private-permissions",
    "--negative-control-compact-key-staged-runner-heartbeat",
    "--negative-control-compact-key-staged-runner-private-permissions",
    "--negative-control-lineage-proof-finalizer-execution-log-sha256",
    "--negative-control-lineage-proof-finalizer-private-permissions",
    "--negative-control-lineage-proof-helper-output-private-permissions",
    "--negative-control-lineage-proof-staged-runner-heartbeat",
    "--negative-control-lineage-proof-staged-runner-private-permissions",
    "--negative-control-localnet-lifecycle-compact-identity-markers",
    "--negative-control-localnet-lifecycle-helper-validation-dir-aliases",
    "--negative-control-localnet-lifecycle-helper-validation-dir-create-failure",
    "--negative-control-localnet-lifecycle-helper-validation-strict-json",
    "--negative-control-localnet-lifecycle-helper-validation-size-limit",
    "--negative-control-localnet-lifecycle-helper-validation-temp-cleanup-after-write-failure",
    "--negative-control-localnet-lifecycle-helper-validation-temp-cleanup-failure",
    "--negative-control-localnet-lifecycle-helper-validation-temp-cleanup-sync-failure",
    "--negative-control-localnet-lifecycle-helper-validation-temp-cleanup-identity",
    "--negative-control-localnet-lifecycle-helper-validation-temp-write-failure",
    "--negative-control-localnet-lifecycle-helper-acceptance-size-cap-specificity",
    "--negative-control-localnet-lifecycle-helper-final-write-size-cap-specificity",
    "--negative-control-localnet-lifecycle-evidence-size-cap-specificity",
    "--negative-control-localnet-lifecycle-identity-markers",
    "--negative-control-localnet-lifecycle-localnet-markers",
    "--negative-control-localnet-lifecycle-mainnet-markers",
    "--negative-control-localnet-lifecycle-peer-order",
    "--negative-control-release-bundle-android-artifact-root-paths",
    "--negative-control-release-bundle-android-d2d-primary-handoff-path",
    "--negative-control-release-bundle-android-d2d-transcript-path-uniqueness",
    "--negative-control-release-bundle-android-d2d-transport-list-canonical",
    "--negative-control-release-bundle-localnet-counts",
    "--negative-control-release-bundle-localnet-identity",
    "--negative-control-release-bundle-localnet-manifest-hash-distinct",
    "--negative-control-release-bundle-localnet-placeholder-hash",
    "--negative-control-release-bundle-section-empty-digests",
    "--negative-control-release-bundle-section-digest-distinct",
    "--negative-control-release-bundle-localnet-summary-hash-distinct",
    "--negative-control-release-bundle-manifest-timestamp-bound",
    "--negative-control-workflow-negative-control-matrix",
    "--negative-control-workflow-negative-control-handler-duplicates",
    "--negative-control-workflow-negative-control-requirement-duplicates",
    "--negative-control-workflow-negative-control-duplicates",
    "--negative-control-staged-resource-guard-workflow-path",
    "--negative-control-workflow",
    "--negative-control-lineage-proof-timestamp-raw",
    "--negative-control-lineage-proof-readiness-direct-hash-shape",
    "--negative-control-lineage-proof-readiness-direct-hash-read-failure",
    "--negative-control-sdk-default",
    "--negative-control-sdk-default-cross-sdk",
    "--negative-control-v3-release-inventory",
    "--negative-control-v3-native-ingest",
    "--negative-control-v3-legacy-mode",
    "--negative-control-readiness-script-configured-default-wording",
    "--negative-control-readiness-script-abi6-recursive-unavailable-mode",
    "--negative-control-pallas-envelope-type",
    "--negative-control-staged-path-aliases",
    "--negative-control-compact-key-command-canonical",
    "--negative-control-compact-key-scalar-types",
    "--negative-control-compact-key-timestamp-raw",
    "--negative-control-compact-key-evidence-filename",
    "--negative-control-compact-key-evidence-filename-preflight",
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
    "--negative-control-android-device-lab-capture-adb-preflight-call",
    "--negative-control-android-device-lab-capture-android-serial-env-scrub",
    "--negative-control-android-device-lab-capture-non-disruptive-commands",
    "--negative-control-android-device-lab-command-gate-casefold",
    "--negative-control-android-device-lab-capture-adb-state-exactness",
    "--negative-control-android-device-lab-capture-adb-state-detail",
    "--negative-control-android-device-lab-capture-path-component-whitespace",
    "--negative-control-android-device-lab-capture-signer-input-preflight",
    "--negative-control-android-device-lab-capture-attestation-result-binding",
    "--negative-control-android-device-lab-capture-chain-binding",
    "--negative-control-android-device-lab-capture-summary-parent-sync-identity",
    "--negative-control-android-device-lab-capture-summary-published-cleanup-identity",
    "--negative-control-android-device-lab-capture-summary-temp-cleanup-identity",
    "--negative-control-android-device-lab-capture-summary-cleanup-sync-failure",
    "--negative-control-android-device-lab-cli-secret-paths",
    "--negative-control-android-device-lab-d2d-transcript",
    "--negative-control-android-device-lab-d2d-path-root",
    "--negative-control-android-device-lab-d2d-transcript-map-binding",
    "--negative-control-android-device-lab-summary-d2d-transcript-map-binding",
    "--negative-control-android-device-lab-d2d-handoff-path",
    "--negative-control-android-device-lab-summary-d2d-handoff-path",
    "--negative-control-android-device-lab-summary-artifact-root-paths",
    "--negative-control-android-device-lab-d2d-transport-list-canonical",
    "--negative-control-android-device-lab-summary-d2d-transport-list-canonical",
    "--negative-control-android-device-lab-d2d-queue-is-file-preflight",
    "--negative-control-android-device-lab-digest-artifact-file-metadata-failure",
    "--negative-control-android-device-lab-direct-helper-slot-secret-paths",
    "--negative-control-android-device-lab-relative-path-component-whitespace",
    "--negative-control-android-device-lab-direct-helper-slot-path-whitespace",
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
    "--negative-control-android-device-lab-json-output-published-cleanup-sync-failure",
    "--negative-control-android-device-lab-json-output-readback-verification",
    "--negative-control-android-device-lab-json-output-readback-failure",
    "--negative-control-android-device-lab-json-output-readback-size-limit",
    "--negative-control-android-device-lab-json-output-readback-open-path-binding",
    "--negative-control-android-device-lab-json-output-size-limit",
    "--negative-control-android-device-lab-json-output-strict-json-write",
    "--negative-control-android-device-lab-json-output-temp-cleanup-failure",
    "--negative-control-android-device-lab-json-output-temp-cleanup-sync-failure",
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
    "--negative-control-android-device-lab-slot-assembler-published-cleanup-sync-failure",
    "--negative-control-android-device-lab-slot-assembler-copy-readback",
    "--negative-control-android-device-lab-slot-assembler-json-parent-sync-identity",
    "--negative-control-android-device-lab-slot-assembler-json-readback",
    "--negative-control-android-device-lab-slot-assembler-json-temp-cleanup-sync-failure",
    "--negative-control-android-device-lab-slot-assembler-json-temp-cleanup-identity",
    "--negative-control-android-device-lab-slot-assembler-publish-root-identity",
    "--negative-control-android-device-lab-slot-assembler-publish-stage-identity",
    "--negative-control-android-device-lab-slot-assembler-temp-cleanup-identity",
    "--negative-control-android-device-lab-slot-assembler-temp-cleanup-report",
    "--negative-control-android-device-lab-slot-assembler-temp-cleanup-sync-failure",
    "--negative-control-android-device-lab-test-workflow",
    "--negative-control-android-device-lab-format-control-sanitization",
    "--negative-control-android-device-lab-wallet-integrity",
    "--negative-control-android-device-lab-unique-bindings",
    "--negative-control-android-device-lab-d2d-duplicate-bindings",
    "--negative-control-android-device-lab-summary",
    "--negative-control-android-device-lab-summary-complete-evidence",
    "--negative-control-android-device-lab-summary-slot-pruning",
    "--negative-control-android-device-lab-summary-trusted-signer-binding",
    "--negative-control-android-device-lab-summary-zero-trusted-signer-digest",
    "--negative-control-android-device-lab-trusted-signer-map-path-type",
    "--negative-control-android-device-lab-trusted-signer-map-container",
    "--negative-control-android-device-lab-trusted-signer-map-mixed-key-sort",
    "--negative-control-android-device-lab-trusted-signer-map-digest-binding",
    "--negative-control-android-device-lab-trusted-signer-private-key-material",
    "--negative-control-android-device-lab-trusted-signer-input-shape",
    "--negative-control-android-device-lab-missing-trusted-signer-return",
    "--negative-control-android-device-lab-trusted-signer-cli-path-aliases",
    "--negative-control-android-device-lab-cli-path-whitespace",
    "--negative-control-android-device-lab-json-output-cli-path-aliases",
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
    "--negative-control-android-signed-evidence-freshness-matrix-coverage",
    "--negative-control-android-signed-evidence-timestamp-raw",
    "--negative-control-android-signed-evidence-summary-partial-identity",
    "--negative-control-android-signed-evidence-summary-partial-artifact-binding",
    "--negative-control-android-signed-evidence-summary-partial-core-binding",
    "--negative-control-android-signed-evidence-summary-incomplete-entry",
    "--negative-control-android-signed-evidence-summary-slot-id",
    "--negative-control-android-signed-evidence-summary-trusted-signer",
    "--negative-control-android-slot-summary-incomplete-kagemusha",
    "--negative-control-android-duplicate-bindings-incomplete-slot-summary",
    "--negative-control-android-device-lab-metadata-artifact-digest-preflight",
    "--negative-control-android-device-lab-metadata-artifact-open-path-binding",
    "--negative-control-android-device-lab-metadata-artifact-read-failure",
    "--negative-control-android-device-lab-metadata-artifact-size-limit",
    "--negative-control-android-device-lab-minimum-os",
    "--negative-control-android-device-lab-nonfinite-json-constants",
    "--negative-control-android-device-lab-openssl-env-scrub",
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
    "--negative-control-android-device-lab-signer-key-size-limit",
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
    "--negative-control-android-attestation-report-output-path-aliases",
    "--negative-control-android-attestation-report-slot-id-canonical",
    "--negative-control-android-attestation-report-identity-canonical",
    "--negative-control-android-attestation-report-strongbox-level-canonical",
    "--negative-control-android-attestation-report-chain-length-binding",
    "--negative-control-android-device-lab-zero-sha256-placeholders",
    "--negative-control-android-device-lab-source-zero-sha256-placeholders",
    "--negative-control-android-device-lab-apk-code-path-digest-exactness",
    "--negative-control-android-device-lab-release-apk-binding",
    "--negative-control-android-device-lab-signed-harness-result",
    "--negative-control-android-device-lab-child-path-root-aliases",
    "--negative-control-android-device-lab-signer-root-paths",
    "--negative-control-android-device-lab-signer-release-apk-path-root",
    "--negative-control-android-device-lab-signed-evidence-path-root",
    "--negative-control-android-device-lab-release-apk-path-root",
    "--negative-control-android-device-lab-signed-evidence-digest-path-roots",
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
    "--negative-control-android-device-lab-signer-key-path-whitespace",
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
    "--negative-control-android-device-lab-signing-helper-output-whitespace",
    "--negative-control-android-device-lab-signing-helper-metadata-output-whitespace",
    "--negative-control-android-device-lab-signing-helper-cli-output-aliases",
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
    "--negative-control-android-device-lab-signing-helper-published-cleanup-sync-failure",
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
    "--negative-control-android-device-lab-signing-helper-temp-cleanup-sync-failure",
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
    "--negative-control-android-device-lab-raw-puller-install-cleanup-sync-failure",
    "--negative-control-android-device-lab-raw-puller-temp-cleanup-identity",
    "--negative-control-android-device-lab-raw-puller-temp-cleanup-report",
    "--negative-control-android-device-lab-raw-puller-temp-cleanup-sync-failure",
    "--negative-control-android-device-lab-raw-puller-install-rename-dir-fd",
    "--negative-control-android-device-lab-raw-puller-install-output-root-identity",
    "--negative-control-android-device-lab-raw-puller-install-cleanup-dir-fd",
    "--negative-control-android-device-lab-raw-puller-install-slot-entry-dir-fd",
    "--negative-control-android-device-lab-raw-puller-non-disruptive-commands",
    "--negative-control-android-device-lab-raw-puller-path-aliases",
    "--negative-control-android-device-lab-raw-puller-allowed-artifacts",
    "--negative-control-android-device-lab-raw-puller-directory-collision",
    "--negative-control-android-device-lab-raw-puller-entry-cap",
    "--negative-control-android-device-lab-raw-puller-adb-detail-redaction",
    "--negative-control-android-device-lab-raw-puller-summary-strict-json",
    "--negative-control-android-device-lab-raw-puller-summary-size-limit",
    "--negative-control-android-device-lab-raw-puller-summary-parent-sync",
    "--negative-control-android-device-lab-raw-puller-summary-parent-identity",
    "--negative-control-android-device-lab-raw-puller-summary-readback-symlink",
    "--negative-control-android-device-lab-raw-puller-summary-readback-hardlink",
    "--negative-control-android-device-lab-raw-puller-summary-readback-identity",
    "--negative-control-android-device-lab-raw-puller-summary-private-permissions",
    "--negative-control-android-device-lab-raw-puller-summary-temp-cleanup-identity",
    "--negative-control-android-device-lab-raw-puller-summary-temp-cleanup-sync-failure",
    "--negative-control-android-device-lab-raw-puller-published-cleanup-identity",
    "--negative-control-android-device-lab-raw-puller-published-cleanup-sync-failure",
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
    "--negative-control-android-device-lab-raw-puller-result-chain-digest-guarded-read",
    "--negative-control-android-device-lab-raw-puller-text-read-open-identity",
    "--negative-control-android-device-lab-raw-puller-result-challenge-digest-required",
    "--negative-control-android-device-lab-raw-puller-result-closed-schema",
    "--negative-control-android-device-lab-raw-puller-result-identity-strings",
    "--negative-control-android-device-lab-raw-puller-result-sdk-digests",
    "--negative-control-android-device-lab-raw-puller-result-strongbox-levels",
    "--negative-control-android-device-lab-raw-puller-private-permissions",
    "--negative-control-android-device-lab-attestation-report-writer-physical-device",
    "--negative-control-android-device-lab-attestation-report-writer-output-early-preflight",
    "--negative-control-android-device-lab-attestation-report-writer-parent-sync-identity",
    "--negative-control-android-device-lab-attestation-report-writer-published-cleanup-identity",
    "--negative-control-android-device-lab-attestation-report-writer-published-cleanup-sync-failure",
    "--negative-control-android-device-lab-attestation-report-writer-temp-cleanup-failure",
    "--negative-control-android-device-lab-attestation-report-writer-temp-cleanup-sync-failure",
    "--negative-control-android-device-lab-attestation-report-writer-temp-cleanup-identity",
    "--negative-control-android-device-lab-attestation-report-writer-private-permissions",
    "--negative-control-android-device-lab-slot-assembler-private-permissions",
    "--negative-control-android-device-lab-slot-assembler-source-identity-fallback",
    "--negative-control-android-device-lab-d2d-transport-matrix",
    "--negative-control-android-release-bundle-d2d-declaration-binding",
    "--negative-control-release-bundle-android-d2d-transport-list-shape",
    "--negative-control-release-bundle-android-d2d-transcript-binding-shape",
    "--negative-control-release-bundle-android-root-default-wording",
    "--negative-control-release-bundle-summary-drift",
    "--negative-control-release-bundle-manifest-timestamp-bound",
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
    "--negative-control-release-bundle-release-evidence-binding",
    "--negative-control-release-bundle-compact-generator-log-artifact-binding",
    "--negative-control-abi-fixture-integer-scalars",
    "--negative-control-release-bundle-summary-shape",
    "--negative-control-release-bundle-summary-section-schema",
    "--negative-control-release-bundle-android-signed-evidence-summary-schema",
    "--negative-control-release-bundle-android-slot-entry-shape",
    "--negative-control-release-bundle-android-signed-evidence-entry-shape",
    "--negative-control-release-bundle-android-signed-evidence-path-shape",
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
    "--negative-control-release-bundle-temp-cleanup-sync-failure",
    "--negative-control-release-bundle-temp-cleanup-identity",
    "--negative-control-release-bundle-strict-json-write",
    "--negative-control-release-bundle-output-size-limit",
    "--negative-control-release-bundle-output-readback-failure",
    "--negative-control-release-bundle-output-readback-size-limit",
    "--negative-control-release-bundle-output-readback-open-path-binding",
    "--negative-control-release-bundle-output-private-permissions",
    "--negative-control-release-bundle-output-parent-sync-identity",
    "--negative-control-release-bundle-output-published-cleanup-identity",
    "--negative-control-release-bundle-output-published-cleanup-sync-failure",
    "--negative-control-release-bundle-output-post-write-preflight",
    "--negative-control-release-bundle-control-path-preflight",
    "--negative-control-release-bundle-input-path-preflight",
    "--negative-control-release-bundle-path-component-whitespace",
    "--negative-control-release-bundle-trusted-signer-path-alias-preflight",
    "--negative-control-release-bundle-scan-preflight",
    "--negative-control-release-bundle-output-overwrite",
    "--negative-control-release-bundle-verify-existing",
    "--negative-control-release-bundle-verify-existing-preflight",
    "--negative-control-release-bundle-verify-existing-evidence-path-shape",
    "--negative-control-release-bundle-verify-existing-evidence-digest-uniqueness",
    "--negative-control-release-bundle-verify-existing-evidence-empty-digest",
    "--negative-control-release-bundle-android-summary-binding",
    "--negative-control-release-bundle-android-signed-evidence-summary-binding",
    "--negative-control-release-bundle-android-signed-evidence-binding",
    "--negative-control-release-bundle-android-evidence-inventory-binding",
    "--negative-control-release-bundle-android-signed-evidence-identity",
    "--negative-control-release-bundle-android-slot-summary-identity",
    "--negative-control-release-bundle-android-signed-evidence-identity-drift",
    "--negative-control-release-bundle-android-slot-identity-drift",
    "--negative-control-release-bundle-manifest-android-signed-evidence-identity-binding",
    "--negative-control-release-bundle-manifest-android-slots-binding",
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
    "--negative-control-100tps-profile-pid-safety-test",
    "--negative-control-android-device-lab-capture-adb-diagnostic-state-test",
    "--negative-control-android-device-lab-capture-expected-family-cli-secret-test",
    "--negative-control-android-device-lab-capture-expected-family-cli-value-test",
    "--negative-control-android-device-lab-capture-expected-family-crlf-test",
    "--negative-control-android-device-lab-capture-expected-family-nondisruptive-test",
    "--negative-control-android-device-lab-capture-expected-family-order-test",
    "--negative-control-android-device-lab-capture-expected-family-redaction-test",
    "--negative-control-android-device-lab-capture-expected-family-wrong-device-test",
    "--negative-control-compact-key-finalizer-runner-temp-test",
    "--negative-control-deploy-localnet-pid-safety-test",
    "--negative-control-kagami-localnet-stop-script-pid-safety",
    "--negative-control-lineage-proof-finalizer-runner-temp-test",
    "--negative-control-local-swarm-pid-safety-test",
    "--negative-control-localnet-lifecycle-acceptance-source-document-fields",
    "--negative-control-localnet-lifecycle-acceptance-source-document-hash-shape",
    "--negative-control-localnet-lifecycle-acceptance-source-document-string-safety",
    "--negative-control-localnet-lifecycle-acceptance-source-event-marker",
    "--negative-control-release-bundle-android-manifest-named-duplicate-binding-test",
    "--negative-control-release-bundle-android-manifest-well-formed-duplicate-binding-test",
    "--negative-control-release-bundle-android-max-signed-bound-drift-test",
    "--negative-control-release-bundle-android-min-signed-bound-drift-test",
    "--negative-control-release-bundle-android-summary-named-duplicate-binding-test",
    "--negative-control-release-bundle-android-summary-well-formed-duplicate-binding-test",
    "--negative-control-release-bundle-manifest-android-max-signed-bound-test",
    "--negative-control-release-bundle-manifest-android-min-signed-bound-test",
    "--negative-control-training-script-pid-safety-test",
  ];

  const expectedModeInventory = [
    ...new Set([
      ...expectedModes,
      ...negativeControlModesFromWorkflowRequirements(
        readiness,
        "ci/check_kagemusha_production_readiness.sh",
      ),
    ]),
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_production_readiness.sh",
    expectedModeInventory,
    "Kagemusha production readiness guard",
  );
  const expectedModeSet = new Set(expectedModeInventory);
  assert.equal(
    expectedModeSet.size,
    expectedModeInventory.length,
    "Kagemusha production readiness expectedModes must not contain duplicates",
  );
  const actualModeSet = new Set(
    [...readiness.matchAll(/^if mode == "(--negative-control-[a-z0-9-]+)":/gmu)].map(
      ([, mode]) => mode,
    ),
  );
  assert.deepEqual(
    [...actualModeSet].sort(),
    [...expectedModeSet].sort(),
    "Kagemusha production readiness expectedModes must equal implemented guard modes",
  );
  for (const mode of expectedModeInventory) {
    assert.ok(
      readiness.includes(`ci/check_kagemusha_production_readiness.sh ${mode}`),
      `production readiness workflow requirements must include ${mode}`,
    );
    assert.ok(readiness.includes(`if mode == "${mode}":`), `production readiness guard must implement ${mode}`);
  }
  assert.doesNotMatch(
    readiness,
    /^    raise SystemExit\(0\)\n    raise SystemExit\("negative control failed/gmu,
    "production readiness negative controls must not pass at top level before their failure exit",
  );

  const readinessBranch = (mode) => {
    const start = readiness.indexOf(`if mode == "${mode}":`);
    assert.notEqual(start, -1, `missing readiness branch ${mode}`);
    const end = readiness.indexOf("\nif mode ==", start + 1);
    return readiness.slice(start, end === -1 ? readiness.length : end);
  };

  const testCoverageNegativeControls = [
    [
      "--negative-control-100tps-profile-pid-safety-test",
      "scripts/tests/run_100tps_profile_localnet_test.py",
      "test_load_wait_uses_ps_liveness_without_sigkill",
      "test_load_wait_allows_null_signal_and_sigkill",
    ],
    [
      "--negative-control-android-sample-env-pid-safety-test",
      "scripts/tests/android_sample_env_test.py",
      "test_background_liveness_uses_ps_without_sigkill",
      "test_background_liveness_allows_null_signal_and_sigkill",
    ],
    [
      "--negative-control-ios-demo-pid-safety-test",
      "scripts/tests/ios_demo_start_test.py",
      "test_background_liveness_uses_owned_job_and_ps_without_sigkill",
      "test_background_liveness_allows_null_signal_and_sigkill",
    ],
    [
      "--negative-control-kaigi-demo-pid-safety-test",
      "scripts/tests/kaigi_demo_test.py",
      "test_background_liveness_uses_owned_job_and_ps_without_sigkill",
      "test_background_liveness_allows_null_signal_and_sigkill",
    ],
    [
      "--negative-control-mochi-local-sandbox-pid-safety-test",
      "scripts/tests/mochi_local_sandbox_test.py",
      "test_pidfile_liveness_uses_ps_and_command_ownership",
      "test_pidfile_liveness_allows_pidfile_reuse_without_command_ownership",
    ],
    [
      "--negative-control-android-device-lab-capture-adb-diagnostic-state-test",
      "scripts/tests/check_android_device_lab_slot_test.py",
      "test_android_capture_classifies_adb_devices_diagnostic_states",
      "test_android_capture_ignores_adb_devices_diagnostic_states",
    ],
    [
      "--negative-control-android-device-lab-capture-expected-family-cli-secret-test",
      "scripts/tests/check_android_device_lab_slot_test.py",
      "test_android_capture_expected_family_cli_secret_does_not_leak_before_commands",
      "test_android_capture_expected_family_cli_secret_leaks_before_commands",
    ],
    [
      "--negative-control-android-device-lab-capture-expected-family-cli-value-test",
      "scripts/tests/check_android_device_lab_slot_test.py",
      "test_android_capture_expected_family_rejects_invalid_value_before_commands",
      "test_android_capture_expected_family_accepts_invalid_value_before_commands",
    ],
    [
      "--negative-control-android-device-lab-capture-expected-family-crlf-test",
      "scripts/tests/check_android_device_lab_slot_test.py",
      "test_android_capture_expected_family_preflight_accepts_adb_crlf",
      "test_android_capture_expected_family_preflight_rejects_adb_crlf",
    ],
    [
      "--negative-control-android-device-lab-capture-expected-family-nondisruptive-test",
      "scripts/tests/check_android_device_lab_slot_test.py",
      "test_android_capture_expected_family_rejects_disruptive_getprop_before_runner",
      "test_android_capture_expected_family_accepts_disruptive_getprop_before_runner",
    ],
    [
      "--negative-control-android-device-lab-capture-expected-family-order-test",
      "scripts/tests/check_android_device_lab_slot_test.py",
      "test_android_capture_expected_family_preflight_runs_before_build",
      "test_android_capture_expected_family_preflight_runs_after_build",
    ],
    [
      "--negative-control-android-device-lab-capture-expected-family-redaction-test",
      "scripts/tests/check_android_device_lab_slot_test.py",
      "test_android_capture_expected_family_redacts_unsafe_getprop_before_build",
      "test_android_capture_expected_family_leaks_unsafe_getprop_before_build",
    ],
    [
      "--negative-control-android-device-lab-capture-expected-family-wrong-device-test",
      "scripts/tests/check_android_device_lab_slot_test.py",
      "test_android_capture_expected_family_rejects_wrong_device_before_build",
      "test_android_capture_expected_family_accepts_wrong_device_before_build",
    ],
    [
      "--negative-control-compact-key-finalizer-runner-temp-test",
      "scripts/tests/kagemusha_production_readiness_test.py",
      "test_compact_key_staged_finalizer_rejects_runner_temp_before_exit_marker",
      "test_compact_key_staged_finalizer_accepts_runner_temp_before_exit_marker",
    ],
    [
      "--negative-control-deploy-localnet-pid-safety-test",
      "scripts/tests/deploy_localnet_test.py",
      "test_force_cleanup_uses_guarded_pid_ownership_checks",
      "test_force_cleanup_allows_unguarded_stop_script",
    ],
    [
      "--negative-control-lineage-proof-finalizer-runner-temp-test",
      "scripts/tests/kagemusha_production_readiness_test.py",
      "test_lineage_proof_staged_finalizer_rejects_runner_temp_before_exit_marker",
      "test_lineage_proof_staged_finalizer_accepts_runner_temp_before_exit_marker",
    ],
    [
      "--negative-control-local-swarm-pid-safety-test",
      "scripts/tests/run_local_swarm_test.py",
      "test_stop_guidance_uses_guarded_pid_ownership_checks",
      "test_stop_guidance_allows_raw_pid_kills",
    ],
    [
      "--negative-control-release-bundle-android-manifest-named-duplicate-binding-test",
      "scripts/tests/kagemusha_production_readiness_test.py",
      "test_kagemusha_release_bundle_verify_existing_rejects_named_android_duplicate_binding_inventory",
      "test_kagemusha_release_bundle_verify_existing_accepts_named_android_duplicate_binding_inventory",
    ],
    [
      "--negative-control-release-bundle-android-manifest-well-formed-duplicate-binding-test",
      "scripts/tests/kagemusha_production_readiness_test.py",
      "test_kagemusha_release_bundle_verify_existing_rejects_well_formed_android_duplicate_binding_inventory",
      "test_kagemusha_release_bundle_verify_existing_accepts_well_formed_android_duplicate_binding_inventory",
    ],
    [
      "--negative-control-release-bundle-android-max-signed-bound-drift-test",
      "scripts/tests/kagemusha_production_readiness_test.py",
      "test_kagemusha_release_bundle_rejects_android_max_signed_at_summary_drift",
      "test_kagemusha_release_bundle_accepts_android_max_signed_at_summary_drift",
    ],
    [
      "--negative-control-release-bundle-android-min-signed-bound-drift-test",
      "scripts/tests/kagemusha_production_readiness_test.py",
      "test_kagemusha_release_bundle_rejects_android_min_signed_at_summary_drift",
      "test_kagemusha_release_bundle_accepts_android_min_signed_at_summary_drift",
    ],
    [
      "--negative-control-release-bundle-android-summary-named-duplicate-binding-test",
      "scripts/tests/kagemusha_production_readiness_test.py",
      "test_kagemusha_release_bundle_rejects_named_android_duplicate_binding_inventory",
      "test_kagemusha_release_bundle_accepts_named_android_duplicate_binding_inventory",
    ],
    [
      "--negative-control-release-bundle-android-summary-well-formed-duplicate-binding-test",
      "scripts/tests/kagemusha_production_readiness_test.py",
      "test_kagemusha_release_bundle_rejects_well_formed_android_duplicate_binding_inventory",
      "test_kagemusha_release_bundle_accepts_well_formed_android_duplicate_binding_inventory",
    ],
    [
      "--negative-control-release-bundle-manifest-android-max-signed-bound-test",
      "scripts/tests/kagemusha_production_readiness_test.py",
      "test_kagemusha_release_bundle_verify_existing_rejects_android_max_signed_at_bound_excluding_slot",
      "test_kagemusha_release_bundle_verify_existing_accepts_android_max_signed_at_bound_excluding_slot",
    ],
    [
      "--negative-control-release-bundle-manifest-android-min-signed-bound-test",
      "scripts/tests/kagemusha_production_readiness_test.py",
      "test_kagemusha_release_bundle_verify_existing_rejects_android_min_signed_at_bound_excluding_slot",
      "test_kagemusha_release_bundle_verify_existing_accepts_android_min_signed_at_bound_excluding_slot",
    ],
    [
      "--negative-control-training-script-pid-safety-test",
      "scripts/tests/training_script_2_test.py",
      "test_stop_localnet_uses_guarded_pid_ownership_checks",
      "test_stop_localnet_allows_unguarded_stop_script",
    ],
  ];
  assert.deepEqual(
    [...actualModeSet].filter((mode) => mode.endsWith("-test")).sort(),
    testCoverageNegativeControls.map(([mode]) => mode).sort(),
    "production-readiness test-coverage negative controls must be reviewed explicitly",
  );
  for (const [mode, target, guardedTest, weakenedTest] of testCoverageNegativeControls) {
    const branch = readinessBranch(mode);
    assert.match(branch, /run_negative_control\(/u, `${mode} must use the shared negative-control runner`);
    assertContainsAll(
      branch,
      [
        "override_text(",
        `"${target}"`,
        `"${guardedTest}"`,
        `"${weakenedTest}"`,
      ],
      `${mode} must mutate the exact regression test marker`,
    );
  }

  const androidCommandGateUnitTests = [
    "test_android_capture_command_gate_rejects_process_management",
    "test_android_slot_command_gate_rejects_process_management",
    "test_android_raw_puller_command_gate_rejects_process_management",
  ];
  assertContainsAll(
    readiness,
    androidCommandGateUnitTests,
    "production readiness Android command-gate unittest requirements",
  );
  assertContainsAll(
    readiness,
    [
      '"scripts/kagemusha_android_device_lab_slot.py": (',
      '"ADB_SERIAL_REDACTION"',
      '"def _is_adb_executable(command: Sequence[str]) -> bool:"',
      '"if _is_adb_executable(display_tokens):"',
      '"display_tokens[index + 1] = ADB_SERIAL_REDACTION"',
      '"(\\"am\\", \\"kill\\"),"',
      '"(\\"am\\", \\"kill-all\\"),"',
      '"(\\"cmd\\", \\"activity\\", \\"kill\\"),"',
      '"(\\"cmd\\", \\"activity\\", \\"kill-all\\"),"',
      '"(\\"cmd\\", \\"activity\\", \\"force-stop\\"),"',
      '"(\\"emu\\", \\"kill\\"),"',
      '"(\\"shell\\", \\"stop\\"),"',
      '"(\\"shell\\", \\"start\\"),"',
      '"(\\"setprop\\", \\"sys.powerctl\\"),"',
      '"shutdown",',
      '"poweroff",',
      '"halt",',
    ],
    "production readiness Android slot serial-redaction source requirements",
  );
  assertContainsAll(
    readinessTests,
    [
      ...androidCommandGateUnitTests.map((name) => `def ${name}(self) -> None:`),
      "def test_android_command_gates_reject_package_state_mutations(self) -> None:",
      '["kill", "1234"]',
      '["pkill", "adb"]',
      '["killall", "adb"]',
      '["adb", "kill-server"]',
      '["adb", "disconnect"]',
      '"adb reboot"',
      '"adb root"',
      '"adb unroot"',
      '"adb remount"',
      '"adb emu kill"',
      '"adb power shutdown"',
      '"adb shell stop"',
      '"adb shell start"',
      '"adb powerctl"',
      '"adb am kill"',
      '"adb am kill-all"',
      '"adb cmd activity kill"',
      '"adb cmd activity kill-all"',
      '"shell", "am", "force-stop", "pkg"',
      '"adb cmd activity force-stop"',
      '"pm enable"',
      '"cmd package suspend"',
      '"cmd package unsuspend"',
      '"setprop sys.powerctl"',
      '"svc power shutdown"',
      '"appops reset"',
      '"cmd appops reset"',
      '"mixed-case kill executable path"',
      '"mixed-case adb token"',
      '"mixed-case adb sequence"',
      '"KiLl-SeRvEr"',
      '"FORCE-STOP"',
      '"cmd",',
      '"activity",',
      '"force-stop",',
      'self.assertNotIn("SERIAL-123", rendered)',
      'self.assertNotIn("SERIAL-456", rendered)',
      'self.assertNotIn("SERIAL-789", rendered)',
      '["adb", "-s", "SERIAL-123", "devices", "-l"]',
      '["adb", "-s", "SERIAL-456", "shell", "getprop", "ro.product.model"]',
      '["adb", "-s", "SERIAL-789", "shell", "run-as", "pkg", "cat", "slot"]',
    ],
    "Kagemusha production readiness Android command-gate unit tests",
  );
  for (const [sourceText, label] of [
    [androidCaptureHelper, "Android capture helper"],
    [androidSlotHelper, "Android slot assembler"],
    [androidRawPuller, "Android raw puller"],
  ]) {
    assertContainsAll(
      sourceText,
      [
        'ADB_SERIAL_REDACTION = "<redacted-adb-serial>"',
        "def _is_adb_executable(command: Sequence[str]) -> bool:",
        "if _is_adb_executable(display_tokens):",
        "normalized_tokens = [token.casefold() for token in tokens]",
        'executable = tokens[0].replace("\\\\", "/").rsplit("/", 1)[-1].casefold()',
        "tuple(normalized_tokens[index : index + width]) == sequence",
        '"kill-server",',
        '"reconnect",',
        '"disconnect",',
        '"reboot",',
        '"root",',
        '"unroot",',
        '"remount",',
        '"shutdown",',
        '"poweroff",',
        '"halt",',
        "display_tokens[index + 1] = ADB_SERIAL_REDACTION",
        '("am", "kill")',
        '("am", "kill-all")',
        '("cmd", "activity", "kill")',
        '("cmd", "activity", "kill-all")',
        '("cmd", "activity", "force-stop")',
        '("emu", "kill")',
        '("shell", "stop")',
        '("shell", "start")',
        '("setprop", "sys.powerctl")',
      ],
      `${label} ADB serial redaction`,
    );
  }

  const exactMutationNegativeControls = [
    [
      "--negative-control-kagami-localnet-stop-script-pid-safety",
      [
        "crates/iroha_kagami/src/localnet.rs",
        "stop script should not escalate to SIGKILL",
        "stop script allows SIGKILL escalation",
      ],
      "Kagami generated localnet stop script PID safety",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-source-document-fields",
      [
        "scripts/kagemusha_localnet_lifecycle_acceptance.py",
        "set(document) - expected_fields",
        "source artifact contains unexpected field",
      ],
      "Kagemusha localnet lifecycle source-document field gate",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-source-document-string-safety",
      [
        "scripts/kagemusha_localnet_lifecycle_acceptance.py",
        "readiness.device_lab._contains_control_character(value)",
        "readiness.device_lab.SECRET_RE.search(value)",
        "messages['control']",
        "messages['secret']",
      ],
      "Kagemusha localnet lifecycle source-document string safety gate",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-source-document-hash-shape",
      [
        "scripts/kagemusha_localnet_lifecycle_acceptance.py",
        "override_text_all(",
        '_append_source_hash_errors(errors, document, "tx_hash", flag)',
      ],
      "Kagemusha localnet lifecycle source-document hash-shape gate",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-source-event-marker",
      [
        "scripts/kagemusha_localnet_lifecycle_acceptance.py",
        '_append_json_source_string_errors(errors, document, "event", flag)',
        '"        pass"',
      ],
      "Kagemusha localnet lifecycle source event marker gate",
    ],
  ];
  for (const [mode, expectedFragments, label] of exactMutationNegativeControls) {
    const branch = readinessBranch(mode);
    assert.match(branch, /run_negative_control\(/u, `${label} negative control must use the shared runner`);
    assertContainsAll(branch, expectedFragments, `${label} must mutate the exact guarded source`);
  }

  assertContainsAll(
    readiness,
    ["test_lineage_verifier_witness_profile_matches_data_model_constant"],
    "Kagemusha readiness verifier witness profile guard",
  );
  assertContainsAll(
    readiness,
    [
      verifierWitnessProfile,
      "direct `255 x 1` fixed",
      "direct `255 x 1` verifier profile",
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
      /`artifacts\/kagemusha\/lineage-proof-evidence\.json`,\\n[\s\S]*?""/u,
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
      /direct `255 x 1` verifier profile[\s\S]*?direct `85 x 3` verifier profile[\s\S]*?direct `255 x 1` fixed[\s\S]*?direct `85 x 3` fixed/u,
      "offline Kagemusha verifier-witness profile exactness",
    ],
    [
      "--negative-control-compact-key-release-tooling",
      /write_halo2_ipa_kagemusha_recursive_compact_payment_token_proving_key_archive[\s\S]*?write_halo2_ipa_kagemusha_recursive_compact_payment_token_disabled/u,
      "ABI-7 compact key release tooling",
    ],
    [
      "--negative-control-cli-proof-attachment-envelope-hash",
      /envelope_hash_hex must be provided[\s\S]*?envelope_hash_hex may be omitted[\s\S]*?att\.structural_error\(\)[\s\S]*?None/u,
      "CLI proof attachment envelope hash and structural gate",
    ],
    [
      "--negative-control-connect-proof-attachment-envelope-hash",
      /decode_exact_lower_hex_array[\s\S]*?decode_lenient_hex_array[\s\S]*?attachment\.structural_error\(\)\.is_some\(\)[\s\S]*?false/u,
      "Connect bridge proof attachment envelope hash and structural gate",
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
      /label="ABI-7 recursive compact key evidence",\\n        max_bytes=MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES,[\s\S]*?label="ABI-7 recursive compact key evidence",\\n        max_bytes=None,/u,
      "ABI-7 recursive compact key evidence JSON size limit",
    ],
    [
      "--negative-control-compact-key-readiness-artifact-open-path-binding",
      /expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "ABI-7 recursive compact key readiness artifact open-path binding",
    ],
    [
      "--negative-control-lineage-placeholder-artifacts",
      /must be generated lineage material, not a placeholder fixture[\s\S]*?may use placeholder fixture material/u,
      "Reserved-lineage placeholder artifact gate",
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
      "--negative-control-compact-key-helper-validation-temp-cleanup-sync-failure",
      /recursive compact key evidence validation file cleanup could not be synced[\s\S]*?recursive compact key evidence validation cleanup sync failures ignored/u,
      "ABI-7 recursive compact key evidence helper validation temp cleanup sync-failure gate",
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
      "--negative-control-compact-key-helper-generator-log-filename-preflight",
      /generator_log_path\.name != readiness\.COMPACT_KEY_GENERATOR_LOG_FILENAME[\s\S]*?False and generator_log_path\.name != readiness\.COMPACT_KEY_GENERATOR_LOG_FILENAME/u,
      "ABI-7 recursive compact key evidence helper generator-log filename preflight gate",
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
      "--negative-control-compact-key-helper-scalar-preflight",
      /errors\.extend\(readiness\.validate_compact_key_command\(command\)\)[\s\S]*?if errors:[\s\S]*?return None, errors[\s\S]*?errors\.extend\(validate_artifact_dir_path\(artifact_dir\)\)[\s\S]*?errors\.extend\(readiness\.validate_compact_key_command\(command\)\)[\s\S]*?errors\.extend\(validate_artifact_dir_path\(artifact_dir\)\)/u,
      "ABI-7 recursive compact key evidence helper scalar preflight gate",
    ],
    [
      "--negative-control-compact-key-helper-cli-scalar-preflight",
      /scalar_errors\.extend\(readiness\.validate_compact_key_command\(args\.command\)\)[\s\S]*?return 1[\s\S]*?path_errors\.extend\(validate_output_corridor\(out_path, artifact_dir\)\)[\s\S]*?scalar_errors\.extend\(readiness\.validate_compact_key_command\(args\.command\)\)[\s\S]*?path_errors\.extend\(validate_output_corridor\(out_path, artifact_dir\)\)/u,
      "ABI-7 recursive compact key evidence helper CLI scalar preflight gate",
    ],
    [
      "--negative-control-compact-key-helper-output-early-preflight",
      /early_output_errors = preflight_output_path\(out_path, "--out"\)[\s\S]*?early_output_errors = \[\]/u,
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
      "--negative-control-compact-key-helper-output-temp-cleanup-sync-failure",
      /return \["--out temporary file cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "ABI-7 recursive compact key evidence helper output temp cleanup sync-failure gate",
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
      "--negative-control-compact-key-helper-output-published-cleanup-sync-failure",
      /return \["--out cleanup could not be synced after parent sync failure"\][\s\S]*?return \[\]/u,
      "ABI-7 recursive compact key evidence helper output published cleanup sync gate",
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
      "--negative-control-compact-key-helper-output-corridor-resolve-failure",
      /test_compact_key_output_corridor_rejects_parent_resolve_failure[\s\S]*?test_compact_key_output_corridor_allows_parent_resolve_failure/u,
      "ABI-7 recursive compact key evidence helper output corridor resolve-failure gate",
    ],
    [
      "--negative-control-staged-runner-exit-file-path-shape",
      /kagemusha_run_recursive_compact_keygen_staged\.py[\s\S]*?kagemusha_run_lineage_proof_staged\.py[\s\S]*?exit_path_errors = validate_exit_file_path_shape\(args\.exit_file\)[\s\S]*?exit_path_errors = \[\]/u,
      "Kagemusha staged runner exit-file path-shape gate",
    ],
    [
      "--negative-control-staged-finalizer-exit-file-path-shape",
      /exit_path_errors = validate_exit_file_path_shape\(args\.exit_file\)[\s\S]*?exit_path_errors = \[\]/u,
      "Kagemusha staged finalizer exit-file path-shape gate",
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
      "--negative-control-compact-key-finalizer-publish-rollback-cleanup-sync-failure",
      /return \[f"\{label\} rollback cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "ABI-7 recursive compact key staged finalizer publish rollback cleanup sync gate",
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
      "--negative-control-compact-key-finalizer-temp-cleanup-sync-failure",
      /return \["staged finalizer temporary directory cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "ABI-7 recursive compact staged finalizer temporary cleanup sync gate",
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
      "--negative-control-compact-key-staged-runner-published-cleanup-sync-failure",
      /return \[sync_failure_message or failure_message\][\s\S]*?return \[\]/u,
      "ABI-7 recursive compact key staged runner published cleanup sync gate",
    ],
    [
      "--negative-control-compact-key-staged-runner-replace-cleanup-sync-failure",
      /sync_failure_message=f"\{label\} cleanup could not be synced"[\s\S]*?sync_failure_message=None/u,
      "ABI-7 recursive compact key staged runner replace cleanup sync gate",
    ],
    [
      "--negative-control-compact-key-staged-runner-temp-cleanup-sync-failure",
      /sync_failure_message=f"\{label\} temporary output cleanup could not be synced"[\s\S]*?sync_failure_message=None/u,
      "ABI-7 recursive compact key staged runner temporary cleanup sync gate",
    ],
    [
      "--negative-control-compact-key-staged-runner-child-log-file",
      /stdout=log_handle[\s\S]*?stdout=subprocess\.PIPE/u,
      "ABI-7 recursive compact key staged runner child log-file binding",
    ],
    [
      "--negative-control-compact-key-staged-runner-supervisor-output-pipe",
      /scripts\/kagemusha_staged_resource_guard\.py[\s\S]*?process\.wait\(timeout=timeout\)[\s\S]*?process\.wait\(\)/u,
      "ABI-7 recursive compact key staged runner supervisor output pipe",
    ],
    [
      "--negative-control-staged-runner-heavy-job-lock",
      /kagemusha_run_lineage_proof_staged\.py[\s\S]*?kagemusha_run_recursive_compact_keygen_staged\.py[\s\S]*?lock_context = resource_guard\.acquire_heavy_job_lock\(args\.resource_lock_file\)[\s\S]*?lock_context = contextlib\.nullcontext\(\)/u,
      "Kagemusha staged runner heavy-job lock gate",
    ],
    [
      "--negative-control-staged-runner-rss-limit-termination",
      /scripts\/kagemusha_staged_resource_guard\.py[\s\S]*?if last_rss_bytes > max_rss_bytes:[\s\S]*?if False and last_rss_bytes > max_rss_bytes:/u,
      "Kagemusha staged resource guard RSS termination gate",
    ],
    [
      "--negative-control-staged-runner-residual-process-group",
      /scripts\/kagemusha_staged_resource_guard\.py[\s\S]*?if residual_rss_bytes > 0:[\s\S]*?if False and residual_rss_bytes > 0:/u,
      "Kagemusha staged resource guard residual process-group gate",
    ],
    [
      "--negative-control-staged-runner-existing-heavy-job-conflict",
      /kagemusha_run_lineage_proof_staged\.py[\s\S]*?kagemusha_run_recursive_compact_keygen_staged\.py[\s\S]*?conflict_errors = resource_guard\.validate_no_conflicting_heavy_jobs\(\)[\s\S]*?conflict_errors = \[\]/u,
      "Kagemusha staged runner existing heavy-job conflict gate",
    ],
    [
      "--negative-control-staged-runner-resource-report-fields",
      /kagemusha_run_lineage_proof_staged\.py[\s\S]*?kagemusha_run_recursive_compact_keygen_staged\.py[\s\S]*?\*\*resource_summary\.report_fields\(\),[\s\S]*?"",/u,
      "Kagemusha staged runner resource report fields gate",
    ],
    [
      "--negative-control-staged-runner-relative-repo-root-child-path",
      /kagemusha_run_lineage_proof_staged\.py[\s\S]*?kagemusha_run_recursive_compact_keygen_staged\.py[\s\S]*?return repo_root if repo_root\.is_absolute\(\) else Path\.cwd\(\) \/ repo_root[\s\S]*?return repo_root/u,
      "Kagemusha staged runner relative repo-root child PATH",
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
      "--negative-control-compact-key-staged-runner-resume-artifact-prefix",
      /content_errors = readiness\.validate_compact_key_artifact_prefix\(prefix, artifact\)[\s\S]*?content_errors = \[\]/u,
      "ABI-7 recursive compact key staged runner resume artifact-prefix gate",
    ],
    [
      "--negative-control-doc-route",
      /roadmap\.md[\s\S]*?Reserved-lineage recursive spend path[\s\S]*?semantic aggregation compact path/u,
      "production route docs",
    ],
    [
      "--negative-control-evidence-helper-path-aliases",
      /evidence_helper_alias_checks[\s\S]*?must not contain surrounding whitespace[\s\S]*?must not contain backslashes[\s\S]*?must be canonical[\s\S]*?kagemusha_lineage_proof_evidence\.py[\s\S]*?kagemusha_recursive_compact_key_evidence\.py/u,
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
      /_path_has_surrounding_whitespace_component\([\s\S]*?path[\s\S]*?path must not contain surrounding whitespace[\s\S]*?path must not contain backslashes[\s\S]*?path must be canonical[\s\S]*?release_json_ancestor_errors = device_lab\.validate_no_symlink_ancestors/u,
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
      "--negative-control-kagemusha-readiness-evidence-direct-secret-paths",
      /test_readiness_cli_rejects_evidence_secret_and_control_paths_before_rollup[\s\S]*?test_readiness_cli_allows_evidence_secret_and_control_paths_before_rollup/u,
      "Kagemusha readiness evidence direct secret/control path gate",
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
      /if not trusted_signer_public_keys[\s\S]*?if False and not trusted_signer_public_keys/u,
      "Kagemusha production readiness evidence rollup",
    ],
    [
      "--negative-control-kagemusha-readiness-rollup-path-safety",
      /path_blockers = validate_cli_path_arguments\(args\)[\s\S]*?path_blockers = \[\]/u,
      "Kagemusha readiness rollup path safety",
    ],
    [
      "--negative-control-kagemusha-readiness-cli-path-whitespace",
      /return blocker\(code, f"\{label\} must not contain surrounding whitespace"\)[\s\S]*?return None/u,
      "Kagemusha readiness CLI path whitespace preflight",
    ],
    [
      "--negative-control-kagemusha-readiness-cli-path-component-whitespace",
      /_path_has_surrounding_whitespace_component\(root\)[\s\S]*?_path_has_surrounding_whitespace_component\(candidate\)[\s\S]*?""/u,
      "Kagemusha readiness CLI path component whitespace preflight",
    ],
    [
      "--negative-control-kagemusha-readiness-source-marker-direct-secret-paths",
      /def _validate_repo_source_marker_file_for_read[\s\S]*?SECRET_RE\.search\(path_text\)[\s\S]*?\{label\} path must not contain secret-looking material[\s\S]*?def _validate_repo_source_marker_file_for_read/u,
      "Kagemusha readiness source marker direct secret-path gate",
    ],
    [
      "--negative-control-kagemusha-readiness-source-marker-direct-path-aliases",
      /_path_has_surrounding_whitespace_component\([\s\S]*?path[\s\S]*?path must not contain surrounding whitespace[\s\S]*?path must not contain backslashes[\s\S]*?path must be canonical[\s\S]*?errors = \[/u,
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
      "--negative-control-kagemusha-readiness-missing-trusted-signer-before-slot-discovery",
      /return \{[\s\S]*?"missing_device_families": missing_device_families[\s\S]*?ignored_missing_trusted_signer_summary = \{/u,
      "Kagemusha readiness missing trusted signer before Android slot discovery",
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
      "--negative-control-kagemusha-readiness-summary-output-cli-path-aliases",
      /if label != "--repo-root":[\s\S]*?_cli_path_shape_blocker\(value, label=label, code=code\)[\s\S]*?if label not in \("--repo-root", "--summary-out"\):/u,
      "Kagemusha readiness summary output CLI path-alias preflight",
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
      "--negative-control-kagemusha-readiness-summary-output-temp-cleanup-sync-failure",
      /"--summary-out temporary file cleanup could not be synced"[\s\S]*?"--summary-out temp cleanup sync is optional"/u,
      "Kagemusha readiness summary output temp cleanup sync gate",
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
      "--negative-control-kagemusha-readiness-summary-output-published-cleanup-sync-failure",
      /"--summary-out cleanup could not be synced after parent sync failure"[\s\S]*?"--summary-out cleanup sync ignored"/u,
      "Kagemusha readiness summary output published cleanup sync gate",
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
      "--negative-control-abi7-compact-package-only-verifier-dispatch",
      /packaged_vk_for![\s\S]*?cached_vk_for!/u,
      "ABI-7 compact package-only verifier dispatch",
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
      "--negative-control-localnet-lifecycle-evidence",
      /localnet_lifecycle_evidence_missing[\s\S]*?localnet_lifecycle_evidence_optional/u,
      "Kagemusha localnet lifecycle evidence",
    ],
    [
      "--negative-control-localnet-lifecycle-evidence-path-aliases",
      /localnet_lifecycle_evidence_path=localnet_lifecycle_evidence_path,[\s\S]*?localnet_lifecycle_evidence_path=localnet_lifecycle_evidence_path\.resolve\(\),/u,
      "Kagemusha localnet lifecycle evidence path alias gate",
    ],
    [
      "--negative-control-localnet-lifecycle-evidence-filename",
      /localnet_lifecycle_evidence_filename[\s\S]*?localnet_lifecycle_evidence_any_filename/u,
      "Kagemusha localnet lifecycle evidence filename gate",
    ],
    [
      "--negative-control-localnet-lifecycle-evidence-filename-preflight",
      /return \{[\s\S]*?"path": LOCALNET_LIFECYCLE_EVIDENCE_SUMMARY_LABEL[\s\S]*?_ignored_filename_details = \{[\s\S]*?"path": LOCALNET_LIFECYCLE_EVIDENCE_SUMMARY_LABEL/u,
      "Kagemusha localnet lifecycle evidence filename preflight gate",
    ],
    [
      "--negative-control-localnet-lifecycle-evidence-size-cap-specificity",
      /label="Kagemusha localnet lifecycle evidence",\\n\s*max_bytes=MAX_LOCALNET_LIFECYCLE_EVIDENCE_JSON_BYTES,[\s\S]*?label="Kagemusha localnet lifecycle evidence",\\n\s*max_bytes=MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES,/u,
      "Kagemusha localnet lifecycle evidence dedicated size cap",
    ],
    [
      "--negative-control-localnet-lifecycle-evidence-adversarial-coverage",
      /test_localnet_lifecycle_evidence_rejects_adversarial_inputs[\s\S]*?test_localnet_lifecycle_evidence_accepts_adversarial_inputs/u,
      "Kagemusha localnet lifecycle adversarial evidence coverage",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-report-path-shape",
      /report_shape_errors = validate_acceptance_report_path_shape\(acceptance_report\)[\s\S]*?report_shape_errors = \[\]/u,
      "Kagemusha localnet lifecycle acceptance-report path-shape gate",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-report-filename",
      /acceptance_report\.name != LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME[\s\S]*?False and acceptance_report\.name != LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME/u,
      "Kagemusha localnet lifecycle acceptance-report filename gate",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-report-filename-preflight",
      /acceptance_report\.name != LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME[\s\S]*?False and acceptance_report\.name != LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME/u,
      "Kagemusha localnet lifecycle acceptance-report filename preflight gate",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-identity-preflight",
      /scripts\/kagemusha_localnet_lifecycle_acceptance\.py[\s\S]*?identity_errors = validate_acceptance_identity\([\s\S]*?return None, identity_errors/u,
      "Kagemusha localnet lifecycle acceptance identity preflight gate",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-source-path-preflight",
      /scripts\/kagemusha_localnet_lifecycle_acceptance\.py[\s\S]*?source_artifact_path_errors = validate_source_artifact_paths\([\s\S]*?return None, source_artifact_path_errors/u,
      "Kagemusha localnet lifecycle acceptance source-artifact path preflight gate",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-source-path-shape-preflight",
      /scripts\/kagemusha_localnet_lifecycle_acceptance\.py[\s\S]*?shape_errors = validate_source_artifact_path_shapes\([\s\S]*?return shape_errors/u,
      "Kagemusha localnet lifecycle acceptance source-artifact shape preflight gate",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-source-identity-preflight",
      /scripts\/kagemusha_localnet_lifecycle_acceptance\.py[\s\S]*?source_artifact_identity_errors = validate_source_artifact_file_identities\([\s\S]*?return None, source_artifact_identity_errors/u,
      "Kagemusha localnet lifecycle acceptance source-artifact identity preflight gate",
    ],
    [
      "--negative-control-localnet-lifecycle-acceptance-output-corridor-before-parent-create",
      /lineage_helper\.validate_output_corridor\(out_path, artifact_dir\)[\s\S]*?return corridor_errors[\s\S]*?lineage_helper\.validate_output_path\(out_path, \\?"--out\\?"\)/u,
      "Kagemusha localnet lifecycle acceptance output corridor before parent-create gate",
    ],
    [
      "--negative-control-localnet-lifecycle-source-recorder-root-preflight",
      /integration_tests\/tests\/zk_confidential_localnet\.rs[\s\S]*?prepare_localnet_lifecycle_source_dir\(root\)\?;[\s\S]*?fs::create_dir_all\(root\)/u,
      "Kagemusha localnet lifecycle source recorder root preflight gate",
    ],
    [
      "--negative-control-localnet-lifecycle-source-recorder-private-publish",
      /integration_tests\/tests\/zk_confidential_localnet\.rs[\s\S]*?write_localnet_lifecycle_source_artifact\(&path, json_text\.as_bytes\(\)\)\?;[\s\S]*?fs::write\(&path, json_text\)\?;/u,
      "Kagemusha localnet lifecycle source recorder private publish gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-validation-size-limit",
      /len\(evidence_text\.encode\(\\"utf-8\\"\)\)[\s\S]*?"0"/u,
      "Kagemusha localnet lifecycle helper validation size limit",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-validation-private-permissions",
      /test_localnet_lifecycle_evidence_document_validator_installs_private_scratch_permissions[\s\S]*?test_localnet_lifecycle_evidence_document_validator_allows_public_scratch_permissions/u,
      "Kagemusha localnet lifecycle helper validation private permissions",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-validation-temp-cleanup-sync-failure",
      /localnet lifecycle evidence validation file cleanup could not be synced[\s\S]*?localnet lifecycle evidence validation cleanup sync failures ignored/u,
      "Kagemusha localnet lifecycle helper validation temp cleanup sync-failure gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-input-corridor-resolve-failure",
      /test_localnet_lifecycle_input_validator_rejects_parent_resolve_failure[\s\S]*?test_localnet_lifecycle_input_validator_allows_parent_resolve_failure/u,
      "Kagemusha localnet lifecycle helper input corridor resolve-failure gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-output-early-preflight",
      /early_output_errors = lineage_helper\.preflight_output_path\(out_path, "--out"\)[\s\S]*?early_output_errors = \[\]/u,
      "Kagemusha localnet lifecycle helper early output preflight gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-output-corridor-resolve-failure",
      /test_localnet_lifecycle_output_corridor_rejects_parent_resolve_failure[\s\S]*?test_localnet_lifecycle_output_corridor_allows_parent_resolve_failure/u,
      "Kagemusha localnet lifecycle helper output corridor resolve-failure gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-output-private-permissions",
      /test_localnet_lifecycle_evidence_helper_writes_private_output[\s\S]*?test_localnet_lifecycle_evidence_helper_allows_public_output/u,
      "Kagemusha localnet lifecycle helper output private permissions",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-output-write-failure",
      /if write_errors:[\s\S]*?return 1[\s\S]*?if \[\]:/u,
      "Kagemusha localnet lifecycle helper output write-failure gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-validation-before-write",
      /validation_errors = validate_evidence_document\(evidence, artifact_dir\)[\s\S]*?if validation_errors:[\s\S]*?return 1[\s\S]*?if \[\]:/u,
      "Kagemusha localnet lifecycle helper validation before final write gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-build-errors-before-validation",
      /evidence, errors = build_evidence\([\s\S]*?if errors:[\s\S]*?return 1[\s\S]*?if \[\]:/u,
      "Kagemusha localnet lifecycle helper build errors before validation gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-input-errors-before-build",
      /path_errors\.extend\(validate_localnet_input_paths\(artifact_dir, acceptance_report\)\)[\s\S]*?if path_errors:[\s\S]*?return 1[\s\S]*?if \[\]:/u,
      "Kagemusha localnet lifecycle helper input errors before build gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-output-errors-before-input",
      /early_output_errors = lineage_helper\.preflight_output_path\(out_path, \\?"--out\\?"\)[\s\S]*?path_errors\.extend\(early_output_errors\)[\s\S]*?if path_errors:[\s\S]*?return 1[\s\S]*?if \[\]:/u,
      "Kagemusha localnet lifecycle helper output errors before input gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-raw-paths-before-path-construction",
      /lineage_helper\._secret_path_error\(args\.artifact_dir, \\?"--artifact-dir\\?"\)[\s\S]*?lineage_helper\._secret_path_error\([\s\S]*?args\.acceptance_report[\s\S]*?lineage_helper\._secret_path_error\(args\.out, \\?"--out\\?"\)[\s\S]*?path_errors = \[\]/u,
      "Kagemusha localnet lifecycle helper raw path preflight gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-acceptance-size-cap-specificity",
      /max_bytes=MAX_LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_JSON_BYTES[\s\S]*?max_bytes=readiness\.MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES/u,
      "Kagemusha localnet lifecycle helper acceptance-report dedicated size cap",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-final-write-size-cap-specificity",
      /max_bytes=readiness\.MAX_LOCALNET_LIFECYCLE_EVIDENCE_JSON_BYTES[\s\S]*?max_bytes=readiness\.MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES/u,
      "Kagemusha localnet lifecycle helper final write size cap",
    ],
    [
      "--negative-control-localnet-lifecycle-evidence-helper",
      /readiness\.check_localnet_lifecycle_evidence\([\s\S]*?readiness\.check_lineage_proof_evidence\(/u,
      "Kagemusha localnet lifecycle evidence helper",
    ],
    [
      "--negative-control-localnet-lifecycle-future-skew",
      /localnet_lifecycle_evidence_future_dated[\s\S]*?localnet_lifecycle_evidence_allows_future_dated/u,
      "Kagemusha localnet lifecycle evidence future-skew gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-scalar-preflight",
      /errors\.extend\([\s\S]*?_validate_generated_at_future_skew\([\s\S]*?if errors:[\s\S]*?return None, errors[\s\S]*?errors\.extend\(validate_localnet_input_paths\(artifact_dir, acceptance_report\)\)[\s\S]*?errors\.extend\([\s\S]*?_validate_generated_at_future_skew\([\s\S]*?errors\.extend\(validate_localnet_input_paths\(artifact_dir, acceptance_report\)\)/u,
      "Kagemusha localnet lifecycle helper scalar preflight gate",
    ],
    [
      "--negative-control-localnet-lifecycle-helper-cli-scalar-preflight",
      /if scalar_errors:[\s\S]*?return 1[\s\S]*?path_errors\.extend\(lineage_helper\.validate_output_corridor\(out_path, artifact_dir\)\)[\s\S]*?if scalar_errors:[\s\S]*?path_errors\.extend\(lineage_helper\.validate_output_corridor\(out_path, artifact_dir\)\)/u,
      "Kagemusha localnet lifecycle helper CLI scalar preflight gate",
    ],
    [
      "--negative-control-localnet-lifecycle-mainnet-markers",
      /CONTRADICTORY_LOCALNET_TEXT_MARKERS[\s\S]*?CONTRADICTORY_LOCALNET_ENVIRONMENT_MARKERS[\s\S]*?CONTRADICTORY_LOCALNET_COMPACT_MARKERS[\s\S]*?CONTRADICTORY_LOCALNET_COMPACT_ENVIRONMENT_MARKERS/u,
      "Kagemusha localnet lifecycle mainnet identity markers",
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
      /label="Reserved-lineage proof evidence",\\n        max_bytes=MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES,[\s\S]*?label="Reserved-lineage proof evidence",\\n        max_bytes=None,/u,
      "Reserved-lineage proof evidence JSON size limit",
    ],
    [
      "--negative-control-lineage-proof-readiness-artifact-open-path-binding",
      /expected_identity = \(expected_stat\.st_dev, expected_stat\.st_ino\)[\s\S]*?expected_identity = \(open_stat\.st_dev, open_stat\.st_ino\)/u,
      "Reserved-lineage proof readiness artifact open-path binding",
    ],
    [
      "--negative-control-lineage-proof-helper-timestamp-raw",
      /generated_at_errors = _validate_generated_at_utc\(generated_at_utc\)[\s\S]*?generated_at_errors = \[\]/u,
      "Reserved-lineage proof evidence helper raw timestamp gate",
    ],
    [
      "--negative-control-lineage-proof-helper-future-skew",
      /_validate_generated_at_future_skew\([\s\S]*?max_generated_at_future_skew_seconds[\s\S]*?_skip_generated_at_future_skew\(/u,
      "Reserved-lineage proof evidence helper future-skew gate",
    ],
    [
      "--negative-control-lineage-proof-helper-scalar-preflight",
      /errors\.extend\(_validate_elapsed_seconds\(elapsed_seconds\)\)[\s\S]*?if errors:[\s\S]*?return None, errors[\s\S]*?errors\.extend\(validate_lineage_input_paths\(artifact_dir, proof_log\)\)[\s\S]*?errors\.extend\(_validate_elapsed_seconds\(elapsed_seconds\)\)[\s\S]*?errors\.extend\(validate_lineage_input_paths\(artifact_dir, proof_log\)\)/u,
      "Reserved-lineage proof evidence helper scalar preflight gate",
    ],
    [
      "--negative-control-lineage-proof-helper-cli-scalar-preflight",
      /scalar_errors\.extend\(_validate_elapsed_seconds\(args\.elapsed_seconds\)\)[\s\S]*?return 1[\s\S]*?path_errors\.extend\(validate_lineage_input_paths\(artifact_dir, proof_log\)\)[\s\S]*?scalar_errors\.extend\(_validate_elapsed_seconds\(args\.elapsed_seconds\)\)[\s\S]*?path_errors\.extend\(validate_lineage_input_paths\(artifact_dir, proof_log\)\)/u,
      "Reserved-lineage proof evidence helper CLI scalar preflight gate",
    ],
    [
      "--negative-control-lineage-proof-helper-strict-json-write",
      /allow_nan=False[\s\S]*?\["--out evidence is not strict JSON"\][\s\S]*?allow_nan=True/u,
      "Reserved-lineage proof evidence helper strict JSON writer",
    ],
    [
      "--negative-control-lineage-proof-helper-output-explicit-size-cap",
      /len\(evidence_text\.encode\(\\"utf-8\\"\)\) > max_bytes[\s\S]*?len\(evidence_text\.encode\(\\"utf-8\\"\)\) > [\s\S]*?readiness\.MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES/u,
      "Reserved-lineage proof evidence helper explicit output size cap",
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
      "--negative-control-lineage-proof-helper-validation-temp-cleanup-sync-failure",
      /lineage proof evidence validation file cleanup could not be synced[\s\S]*?lineage proof evidence validation cleanup sync failures ignored/u,
      "Reserved-lineage proof evidence helper validation temp cleanup sync-failure gate",
    ],
    [
      "--negative-control-lineage-proof-helper-validation-temp-cleanup-identity",
      /_file_identity\(validation_temp_stat\) != expected_identity[\s\S]*?False/u,
      "Reserved-lineage proof evidence helper validation temp cleanup identity",
    ],
    [
      "--negative-control-lineage-proof-helper-proof-log-filename-preflight",
      /proof_log\.name != expected_proof_log_name[\s\S]*?False and proof_log\.name != expected_proof_log_name/u,
      "Reserved-lineage proof evidence helper proof-log filename preflight gate",
    ],
    [
      "--negative-control-lineage-proof-helper-input-corridor",
      /errors\.extend\(validate_lineage_input_paths\(artifact_dir, proof_log\)\)[\s\S]*?errors\.extend\(\[\]\)[\s\S]*?path_errors\.extend\(validate_lineage_input_paths\(artifact_dir, proof_log\)\)[\s\S]*?path_errors\.extend\(\[\]\)/u,
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
      "--negative-control-lineage-proof-helper-output-temp-cleanup-sync-failure",
      /return \["--out temporary file cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Reserved-lineage proof evidence helper output temp cleanup sync-failure gate",
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
      "--negative-control-lineage-proof-helper-output-published-cleanup-sync-failure",
      /return \["--out cleanup could not be synced after parent sync failure"\][\s\S]*?return \[\]/u,
      "Reserved-lineage proof evidence helper output published cleanup sync gate",
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
      "--negative-control-lineage-proof-finalizer-publish-rollback-cleanup-sync-failure",
      /return \[f"\{label\} rollback cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Reserved-lineage proof staged finalizer publish rollback cleanup sync gate",
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
      "--negative-control-lineage-proof-finalizer-temp-cleanup-sync-failure",
      /return \["staged finalizer temporary directory cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Reserved-lineage proof staged finalizer temporary cleanup sync gate",
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
      "--negative-control-lineage-proof-staged-runner-published-cleanup-sync-failure",
      /return \[sync_failure_message or failure_message\][\s\S]*?return \[\]/u,
      "Reserved-lineage proof staged runner published cleanup sync gate",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-replace-cleanup-sync-failure",
      /sync_failure_message=f"\{label\} cleanup could not be synced"[\s\S]*?sync_failure_message=None/u,
      "Reserved-lineage proof staged runner replace cleanup sync gate",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-temp-cleanup-sync-failure",
      /sync_failure_message=f"\{label\} temporary output cleanup could not be synced"[\s\S]*?sync_failure_message=None/u,
      "Reserved-lineage proof staged runner temporary cleanup sync gate",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-child-log-file",
      /stdout=log_handle[\s\S]*?stdout=subprocess\.PIPE/u,
      "Reserved-lineage proof staged runner child log-file binding",
    ],
    [
      "--negative-control-lineage-proof-staged-runner-supervisor-output-pipe",
      /scripts\/kagemusha_staged_resource_guard\.py[\s\S]*?process\.wait\(timeout=timeout\)[\s\S]*?process\.wait\(\)/u,
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
      "--negative-control-lineage-proof-staged-runner-resume-artifact-content",
      /return readiness\.validate_lineage_artifact_content\(path, artifact\)[\s\S]*?return \[\]/u,
      "Reserved-lineage proof staged runner resume artifact-content gate",
    ],
    [
      "--negative-control-lineage-staged-elapsed-file-path-shape",
      /kagemusha_run_lineage_proof_staged\.py[\s\S]*?kagemusha_finalize_lineage_proof_staged_run\.py[\s\S]*?elapsed_path_errors = validate_elapsed_seconds_file_path_shape\([\s\S]*?elapsed_path_errors = \[\]/u,
      "Kagemusha lineage staged elapsed-seconds file path-shape gate",
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
      "--negative-control-lineage-proof-evidence-filename-preflight",
      /return \{[\s\S]*?"path": LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL[\s\S]*?_ignored_filename_details = \{[\s\S]*?"path": LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL/u,
      "Reserved-lineage proof evidence filename preflight gate",
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
      "--negative-control-workflow-negative-control-matrix",
      /production-readiness negative-control workflow matrix[\s\S]*?negative-control-[\s\S]*?unrouted-matrix-probe/u,
      "production-readiness negative-control workflow matrix",
    ],
    [
      "--negative-control-workflow-negative-control-handler-duplicates",
      /production-readiness negative-control handler duplicate gate[\s\S]*?negative-control-workflow/u,
      "production-readiness negative-control handler duplicate gate",
    ],
    [
      "--negative-control-workflow-negative-control-requirement-duplicates",
      /production-readiness negative-control requirement duplicate gate[\s\S]*?requirement \+ "\\n    " \+ requirement/u,
      "production-readiness negative-control requirement duplicate gate",
    ],
    [
      "--negative-control-workflow-negative-control-duplicates",
      /production-readiness negative-control workflow duplicate gate[\s\S]*?workflow_command \+ "\\n          " \+ workflow_command/u,
      "production-readiness negative-control workflow duplicate gate",
    ],
    [
      "--negative-control-staged-finalizer-rss-terminated-report",
      /kagemusha_finalize_lineage_proof_staged_run\.py[\s\S]*?kagemusha_finalize_recursive_compact_key_staged_run\.py[\s\S]*?require_not_terminated=True[\s\S]*?require_not_terminated=False/u,
      "Kagemusha staged finalizer RSS-terminated report gate",
    ],
    [
      "--negative-control-staged-resource-guard-workflow-path",
      /staged resource guard workflow path[\s\S]*?scripts\/kagemusha_staged_resource_guard\.py/u,
      "staged resource guard workflow path",
    ],
    [
      "--negative-control-lineage-proof-timestamp-raw",
      /code_prefix="lineage_proof_evidence"[\s\S]*?label="Reserved-lineage proof evidence"[\s\S]*?SIGNED_AT_UTC_RE\.fullmatch\(generated_at_raw\) is None[\s\S]*?SIGNED_AT_UTC_RE\.fullmatch\(generated_at_raw\.strip\(\)\) is None/u,
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
      /Some\(KAGEMUSHA_RECURSIVE_SPEND_PRODUCT_MODE_V1\)[\s\S]*?"        None",/u,
      "ABI-18 first-release selector",
    ],
    [
      "--negative-control-sdk-default-cross-sdk",
      /KAGEMUSHA_RECURSIVE_SPEND_PRODUCT_MODE_V1[\s\S]*?case recursiveSpend[\s\S]*?RECURSIVE_SPEND[\s\S]*?unsupported_mode/u,
      "cross-SDK recursive_spend_v1 product selector",
    ],
    [
      "--negative-control-v3-release-inventory",
      /exact ten-file V3 release inventory[\s\S]*?manifest-extra\.json/u,
      "exact ten-file V3 release inventory",
    ],
    [
      "--negative-control-v3-native-ingest",
      /complete ABI-18\/V3 native artifact-ingest surface[\s\S]*?artifact_cancel_removed/u,
      "complete ABI-18/V3 native artifact-ingest surface",
    ],
    [
      "--negative-control-v3-legacy-mode",
      /alternate product mode rejection[\s\S]*?case recursiveSpend = [\s\S]*?case recursiveSpendV2/u,
      "alternate product mode rejection",
    ],
    [
      "--negative-control-readiness-script-configured-default-wording",
      /preserving the configured default[\s\S]*?preserving the leg[\s\S]*?acy default/u,
      "Kagemusha readiness script first-release default wording",
    ],
    [
      "--negative-control-readiness-script-abi6-recursive-unavailable-mode",
      /abi6_manifest_recursive_unavailable_mode[\s\S]*?abi6_manifest_fallback_mode/u,
      "Kagemusha readiness script ABI-6 recursive-unavailable blocker code",
    ],
    [
      "--negative-control-pallas-envelope-type",
      /kagemusha_recursive_compact_record_prover_preflights_pallas_archive_before_unavailable[\s\S]*?kagemusha_recursive_compact_record_prover_skips_pallas_archive_before_unavailable/u,
      "ABI-7 compact Pallas envelope preflight type",
    ],
    [
      "--negative-control-staged-path-aliases",
      /staged_alias_checks[\s\S]*?must not contain surrounding whitespace[\s\S]*?kagemusha_run_lineage_proof_staged\.py[\s\S]*?kagemusha_run_recursive_compact_keygen_staged\.py[\s\S]*?kagemusha_finalize_lineage_proof_staged_run\.py[\s\S]*?kagemusha_finalize_recursive_compact_key_staged_run\.py/u,
      "Kagemusha staged path alias gate",
    ],
    [
      "--negative-control-android-device-lab-d2d-transport-matrix",
      new RegExp(
        [
          "if missing_transports or missing_transport_pairs:",
          "[\\s\\S]*?if False and \\(missing_transports or missing_transport_pairs\\):",
          "[\\s\\S]*?if missing_d2d_payment_transports or missing_d2d_payment_transport_pairs:",
          "[\\s\\S]*?if False and \\(missing_d2d_payment_transports or missing_d2d_payment_transport_pairs\\):",
          "[\\s\\S]*?covered_d2d_payment_transports_by_family",
          "[\\s\\S]*?False and list_fields_ok\\.get",
          "[\\s\\S]*?missing_pairs != _expected_android_missing_d2d_payment_transport_pairs\\(",
          "[\\s\\S]*?False and missing_pairs != _expected_android_missing_d2d_payment_transport_pairs\\(",
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
      "--negative-control-release-bundle-android-artifact-root-paths",
      /safe_relative,[\s\S]*?"evidence",[\s\S]*?"and False"[\s\S]*?safe_relative,[\s\S]*?"wallet",[\s\S]*?"and False"[\s\S]*?safe_relative,[\s\S]*?"attestation",[\s\S]*?"and False"[\s\S]*?safe_relative,[\s\S]*?root,[\s\S]*?"and True"[\s\S]*?relative is None[\s\S]*?False and relative is None/u,
      "Kagemusha release bundle Android artifact root path gates",
    ],
    [
      "--negative-control-release-bundle-android-d2d-transport-list-shape",
      /not d2d_transports_all_strings[\s\S]*?False/u,
      "Kagemusha release bundle Android D2D transport list shape",
    ],
    [
      "--negative-control-release-bundle-android-artifact-root-paths",
      new RegExp(
        [
          '"--negative-control-release-bundle-android-artifact-root-paths"',
          '[\\s\\S]*?"evidence"',
          '[\\s\\S]*?"and False"',
          '[\\s\\S]*?"wallet"',
          '[\\s\\S]*?"and False"',
          '[\\s\\S]*?"attestation"',
          '[\\s\\S]*?"and False"',
          '[\\s\\S]*?relative is None',
          '[\\s\\S]*?False and relative is None',
        ].join(""),
        "u",
      ),
      "Kagemusha release bundle Android artifact root path gates",
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
      "--negative-control-compact-key-evidence-filename-preflight",
      /return \{[\s\S]*?"path": COMPACT_KEY_EVIDENCE_SUMMARY_LABEL[\s\S]*?_ignored_filename_details = \{[\s\S]*?"path": COMPACT_KEY_EVIDENCE_SUMMARY_LABEL/u,
      "ABI-7 recursive compact key evidence filename preflight gate",
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
      "--negative-control-android-device-lab-capture-adb-preflight-call",
      /errors = _run_adb_visibility_preflight_with_wait\(args, env=env, runner=runner\)[\s\S]*?errors = \[\]/u,
      "Android capture wrapper ADB visibility preflight call",
    ],
    [
      "--negative-control-android-device-lab-capture-android-serial-env-scrub",
      /env\.pop\(key, None\)[\s\S]*?env\.get\(key\)/u,
      "Android capture wrapper inherited ANDROID_SERIAL scrub",
    ],
    [
      "--negative-control-android-device-lab-capture-non-disruptive-commands",
      /errors = _command_disruption_errors\(command, label\)[\s\S]*?errors = \[\]/u,
      "Android capture wrapper non-disruptive command gate",
    ],
    [
      "--negative-control-android-device-lab-capture-adb-state-exactness",
      /if state != \\"device\\":[\s\S]*?if False:/u,
      "Android capture wrapper ADB state exactness",
    ],
    [
      "--negative-control-android-device-lab-capture-adb-state-detail",
      /message = f\\"\{label\} must report state device, got \{state\}\\"[\s\S]*?return \[f\\"\{label\} must report state device, got \{state\}\\"\]/u,
      "Android capture wrapper ADB state detail redaction",
    ],
    [
      "--negative-control-android-device-lab-capture-path-component-whitespace",
      /_path_has_surrounding_whitespace_component\([\s\S]*?path[\s\S]*?must not contain surrounding whitespace[\s\S]*?""/u,
      "Android capture wrapper path component-whitespace preflight",
    ],
    [
      "--negative-control-android-device-lab-capture-signer-input-preflight",
      /_validate_required_regular_file\(\\n"[\s\S]*?max_bytes=MAX_CAPTURE_SIGNING_KEY_BYTES,\\n"[\s\S]*?"\[\]"/u,
      "Android capture wrapper signer input preflight",
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
      "--negative-control-android-device-lab-capture-summary-cleanup-sync-failure",
      /return \["capture summary output rollback cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Android capture summary cleanup sync gate",
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
      "--negative-control-android-device-lab-d2d-transcript-map-binding",
      /set\(transcripts\) != declared_transports[\s\S]*?False/u,
      "Android readiness D2D transcript map binding",
    ],
    [
      "--negative-control-android-device-lab-summary-d2d-transcript-map-binding",
      /set\(transcripts\) != declared_transports[\s\S]*?False/u,
      "Android scanner D2D transcript map binding",
    ],
    [
      "--negative-control-android-device-lab-d2d-handoff-path",
      /if not device_lab\._safe_relative_path_is_child_of\(  # type: ignore\[attr-defined\][\s\S]*?normalized,[\s\S]*?expected_root,[\s\S]*?\):[\s\S]*?if False and not/u,
      "Android readiness signed-evidence artifact root path gate",
    ],
    [
      "--negative-control-android-device-lab-summary-d2d-handoff-path",
      /_safe_relative_path_is_child_of\(value, "handoff"\)[\s\S]*?True/u,
      "Android scanner D2D transcript handoff path gate",
    ],
    [
      "--negative-control-android-device-lab-summary-artifact-root-paths",
      /_summary_release_artifact_path\(value\)[\s\S]*?_safe_relative_path_is_child_of\(value, root\)[\s\S]*?return _summary_release_artifact_path\(value\)/u,
      "Android scanner summary artifact root path gates",
    ],
    [
      "--negative-control-android-device-lab-d2d-transport-list-canonical",
      /if transports != sorted\(set\(transports\)\):[\s\S]*?if False:/u,
      "Android readiness D2D transport list canonical gate",
    ],
    [
      "--negative-control-android-device-lab-summary-d2d-transport-list-canonical",
      /if transports != sorted\(set\(transports\)\):[\s\S]*?if False:/u,
      "Android scanner D2D transport list canonical gate",
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
      "--negative-control-android-device-lab-relative-path-component-whitespace",
      /part != part\.strip\(\) for part in candidate\.parts[\s\S]*?""/u,
      "Android device-lab relative path component whitespace gate",
    ],
    [
      "--negative-control-android-device-lab-direct-helper-slot-path-whitespace",
      /_path_has_surrounding_whitespace_component\([\s\S]*?slot_path[\s\S]*?slot path must not contain surrounding whitespace[\s\S]*?""/u,
      "Android device-lab direct helper slot path whitespace gate",
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
      /and value != "0" \* 64[\s\S]*?and True/u,
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
      /_path_has_surrounding_whitespace_component\([\s\S]*?path[\s\S]*?\{label\} must not contain surrounding whitespace[\s\S]*?\{label\} must not contain backslashes[\s\S]*?\{label\} must be canonical[\s\S]*?""/u,
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
      "--negative-control-android-device-lab-json-output-published-cleanup-sync-failure",
      /return \["--json-out cleanup could not be synced after parent sync failure"\][\s\S]*?return \[\]/u,
      "Android device-lab JSON summary output published cleanup sync gate",
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
      "--negative-control-android-device-lab-json-output-temp-cleanup-sync-failure",
      /--json-out temporary file cleanup could not be synced[\s\S]*?return \[\]/u,
      "Android device-lab JSON summary output temp cleanup sync gate",
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
      "--negative-control-android-device-lab-slot-assembler-adb-getprop-non-disruptive",
      /errors = _command_disruption_errors\(command, f"ADB getprop \{prop\}"\)[\s\S]*?errors = \[\]/u,
      "Android slot assembler ADB getprop non-disruptive command gate",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-adb-getprop-timeout",
      /timeout=_timeout_arg\(timeout_seconds\),[\s\S]*?# timeout intentionally disabled/u,
      "Android slot assembler ADB getprop timeout gate",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-source-open-binding",
      /open_identity != expected_identity or path_identity != expected_identity[\s\S]*?False/u,
      "Android device-lab slot assembler source open-path binding",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-root-path-aliases",
      /_path_has_surrounding_whitespace_component\([\s\S]*?root[\s\S]*?device-lab root path must not contain surrounding whitespace[\s\S]*?device-lab root path must not contain backslashes[\s\S]*?device-lab root path must be canonical[\s\S]*?""/u,
      "Android device-lab slot assembler root path-alias gate",
    ],
    [
      "--negative-control-android-device-lab-slot-assembler-source-path-aliases",
      /_path_has_surrounding_whitespace_component\([\s\S]*?path[\s\S]*?\{label\} path must not contain surrounding whitespace[\s\S]*?\{label\} path must not contain backslashes[\s\S]*?\{label\} path must be canonical[\s\S]*?""/u,
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
      "--negative-control-android-device-lab-slot-assembler-published-cleanup-sync-failure",
      /return \[f"\{label\} rollback cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Android device-lab slot assembler published cleanup sync gate",
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
      "--negative-control-android-device-lab-slot-assembler-json-temp-cleanup-sync-failure",
      /return \[f"\{label\} temporary output cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Android device-lab slot assembler JSON temp cleanup sync",
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
      "--negative-control-android-device-lab-slot-assembler-temp-cleanup-sync-failure",
      /return \["staged slot temporary directory cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Android device-lab slot assembler temporary cleanup sync gate",
    ],
    [
      "--negative-control-android-device-lab-test-workflow",
      /check_android_device_lab_slot_test\.py[\s\S]*?disabled_check_android_device_lab_slot_test\.py/u,
      "Android device-lab validator workflow",
    ],
    [
      "--negative-control-android-device-lab-format-control-sanitization",
      /unicodedata\.category\(character\) == "Cf"[\s\S]*?""/u,
      "Android device-lab Unicode format-control sanitization",
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
      "--negative-control-android-device-lab-d2d-duplicate-bindings",
      /d2d_payment_transcript_sha256[\s\S]*?Android device-lab production slots must not reuse a D2D payment transcript digest[\s\S]*?Android device-lab production slots may reuse a D2D payment transcript digest/u,
      "Android device-lab D2D duplicate matrix bindings",
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
      "--negative-control-android-device-lab-summary-slot-pruning",
      /output_reports = _summary_reports_for_release_output\(\\n        summary_reports,\\n        require_complete_signed_evidence=require_complete_kagemusha,\\n        trusted_signer_public_key_sha256=trusted_signer_public_key_sha256,\\n    \)[\s\S]*?output_reports = summary_reports/u,
      "Android device-lab summary slot release-field pruning",
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
      "--negative-control-android-device-lab-trusted-signer-map-digest-binding",
      /trusted signer public key digest must match public key DER sha256[\s\S]*?""/u,
      "Android device-lab trusted-signer direct-map digest binding",
    ],
    [
      "--negative-control-android-device-lab-trusted-signer-private-key-material",
      /any\(marker in public_key_bytes for marker in PRIVATE_KEY_PEM_MARKERS\)[\s\S]*?False and any\(marker in public_key_bytes for marker in PRIVATE_KEY_PEM_MARKERS\)/u,
      "Android device-lab trusted-signer private-key material gate",
    ],
    [
      "--negative-control-android-device-lab-trusted-signer-input-shape",
      /public_key_paths, \(str, bytes, bytearray, os\.PathLike\)[\s\S]*?return \(public_key_paths,\), \[\]/u,
      "Android device-lab trusted-signer input-shape gate",
    ],
    [
      "--negative-control-android-device-lab-missing-trusted-signer-return",
      /trusted signer public key required for Kagemusha production evidence[\s\S]*?return details[\s\S]*?trusted signer public key required for Kagemusha production evidence/u,
      "Android device-lab missing trusted-signer early return",
    ],
    [
      "--negative-control-android-device-lab-trusted-signer-cli-path-aliases",
      /must not contain backslashes[\s\S]*?candidate\.parts[\s\S]*?must be a canonical path/u,
      "Android device-lab trusted-signer CLI path-alias gate",
    ],
    [
      "--negative-control-android-device-lab-cli-path-whitespace",
      /path != path\.strip\(\)[\s\S]*?must not contain surrounding whitespace/u,
      "Android device-lab CLI path whitespace gate",
    ],
    [
      "--negative-control-android-device-lab-json-output-cli-path-aliases",
      /args\.json_out is not None:[\s\S]*?_cli_path_alias_errors\(args\.json_out, "--json-out"\)/u,
      "Android device-lab JSON output CLI path-alias gate",
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
      "--negative-control-android-signed-evidence-freshness-matrix-coverage",
      /matrix_reports = \[[\s\S]*?_android_report_signed_evidence_is_fresh\([\s\S]*?matrix_reports = reports/u,
      "Android signed-evidence freshness matrix coverage admission",
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
      "--negative-control-android-signed-evidence-summary-trusted-signer",
      /_android_signed_evidence_summary\(\\n        reports,\\n        trusted_signer_public_key_sha256_set,\\n    \)[\s\S]*?_android_signed_evidence_summary\(reports\)/u,
      "Android signed-evidence readiness summary trusted signer admission",
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
      /private_key_stat = private_key_path\.stat\(\)[\s\S]*?private key hardlink metadata could not be read[\s\S]*?private_key_stat = private_key_path\.stat\(\)/u,
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
      /public_key_stat = public_key_path\.stat\(\)[\s\S]*?\{label\} hardlink metadata could not be read[\s\S]*?public_key_stat = public_key_path\.stat\(\)/u,
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
      /_path_has_surrounding_whitespace_component\([\s\S]*?path[\s\S]*?path must not contain surrounding whitespace[\s\S]*?if "\\\\\\\\" in path_text:[\s\S]*?errors\.append\(f"\{label\} path must be canonical"\)[\s\S]*?""/u,
      "Android attestation report chain source path alias gate",
    ],
    [
      "--negative-control-android-attestation-report-harness-source-path-aliases",
      /result = device_lab\._load_json\(path, "attestation harness result", errors\)[\s\S]*?result = json\.loads\(path\.read_text\(encoding="utf-8"\)\)/u,
      "Android attestation report harness-result source path alias gate",
    ],
    [
      "--negative-control-android-attestation-report-output-path-aliases",
      /_path_has_surrounding_whitespace_component\([\s\S]*?path[\s\S]*?must not contain surrounding whitespace[\s\S]*?must not contain backslashes[\s\S]*?must be canonical[\s\S]*?""/u,
      "Android attestation report output path alias gate",
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
      /== "0" \* 64[\s\S]*?__disabled_zero_sha256_placeholder_gate__[\s\S]*?!= "0" \* 64[\s\S]*?__disabled_zero_sha256_placeholder_gate__/u,
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
      "--negative-control-android-device-lab-release-apk-path-root",
      /slot\.json offline_wallet_apk_path must stay under evidence\/[\s\S]*?slot\.json offline_wallet_apk_path may point outside evidence\//u,
      "Android device-lab release APK artifact path root",
    ],
    [
      "--negative-control-android-device-lab-signed-harness-result",
      /attestation\/harness-result\.json challenge_hex digest must match slot\.json attestation_challenge_sha256[\s\S]*?attestation\/harness-result\.json challenge_hex digest may differ from slot\.json attestation_challenge_sha256/u,
      "Android device-lab signed harness-result contract",
    ],
    [
      "--negative-control-android-device-lab-child-path-root-aliases",
      /return path_text\.startswith\(prefix\) and len\(path_text\) > len\(prefix\)[\s\S]*?return path_text\.startswith\(prefix\)/u,
      "Android device-lab child path root aliases",
    ],
    [
      "--negative-control-android-device-lab-signer-root-paths",
      /not device_lab\._safe_relative_path_is_child_of\([\s\S]*?False and not device_lab\._safe_relative_path_is_child_of\(/u,
      "Android device-lab signer root path gates",
    ],
    [
      "--negative-control-android-device-lab-signer-release-apk-path-root",
      /apk_path_is_under_evidence = \([\s\S]*?apk_path_is_under_evidence = True or \(/u,
      "Android device-lab signer release APK path root",
    ],
    [
      "--negative-control-android-device-lab-signed-evidence-path-root",
      /slot\.json signed_evidence_artifact_path must stay under evidence\/[\s\S]*?slot\.json signed_evidence_artifact_path may point outside evidence\//u,
      "Android device-lab signed evidence artifact path root",
    ],
    [
      "--negative-control-android-device-lab-signed-evidence-digest-path-roots",
      /if _safe_relative_path_is_child_of\(relative, root\):[\s\S]*?if relative == root or _safe_relative_path_is_child_of\(relative, root\):/u,
      "Android device-lab signed evidence digest path roots",
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
      "--negative-control-android-device-lab-openssl-env-scrub",
      /"LD_PRELOAD"[\s\S]*?"LD_PRELOAD_DISABLED"[\s\S]*?env\.pop\(key, None\)[\s\S]*?env\.get\(key\)[\s\S]*?env=device_lab\._openssl_child_env\(\),[\s\S]*?env=os\.environ\.copy\(\),/u,
      "Android device-lab OpenSSL child environment scrub",
    ],
    [
      "--negative-control-android-device-lab-signer-key-size-limit",
      /public_key_stat\.st_size > MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES[\s\S]*?False and public_key_stat\.st_size > MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES[\s\S]*?private_key_stat\.st_size > device_lab\.MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES[\s\S]*?False and private_key_stat\.st_size > device_lab\.MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES/u,
      "Android device-lab signer key size limit",
    ],
    [
      "--negative-control-android-device-lab-signer-key-path-whitespace",
      /path_text != path_text\.strip\(\)[\s\S]*?_path_has_surrounding_whitespace_component\(path\)[\s\S]*?must not contain surrounding whitespace/u,
      "Android device-lab signer key path whitespace gate",
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
      "--negative-control-android-device-lab-signing-helper-output-whitespace",
      /_path_has_surrounding_whitespace_component\(candidate\)[\s\S]*?""/u,
      "Android device-lab signing helper output whitespace gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-metadata-output-whitespace",
      /_path_has_surrounding_whitespace_component\([\s\S]*?Path\(metadata_output\)[\s\S]*?""/u,
      "Android device-lab signing helper metadata output whitespace gate",
    ],
    [
      "--negative-control-android-device-lab-signing-helper-cli-output-aliases",
      /\*_explicit_output_arg_errors\(output\),[\s\S]*?""/u,
      "Android device-lab signing helper CLI output-alias preflight gate",
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
      "--negative-control-android-device-lab-signing-helper-published-cleanup-sync-failure",
      /return \[f"\{label\} cleanup could not be synced after parent sync failure"\][\s\S]*?return \[\]/u,
      "Android device-lab signed evidence helper published cleanup sync gate",
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
      "--negative-control-android-device-lab-signing-helper-temp-cleanup-sync-failure",
      /return \[f"\{label\} temporary file cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Android device-lab signed evidence helper temp cleanup sync gate",
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
      /root_exists, root_errors = device_lab\.classify_device_lab_root_path\([\s\S]*?candidate_root[\s\S]*?root_exists = candidate_root\.exists\(\)[\s\S]*?root_errors = \[\]/u,
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
      "--negative-control-android-device-lab-raw-puller-non-disruptive-commands",
      /errors = _command_disruption_errors\(command, "latest raw slot ADB query"\)[\s\S]*?errors = \[\]  # _command_disruption_errors\(command, "latest raw slot ADB query"\)[\s\S]*?errors = _command_disruption_errors\(command, "raw slot tar ADB pull"\)[\s\S]*?errors = \[\]  # _command_disruption_errors\(command, "raw slot tar ADB pull"\)/u,
      "Android raw puller non-disruptive command gate",
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
      "--negative-control-android-device-lab-raw-puller-install-cleanup-sync-failure",
      /return \["raw slot partial install cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Android raw puller install cleanup sync gate",
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
      "--negative-control-android-device-lab-raw-puller-temp-cleanup-sync-failure",
      /return \["raw pull temporary directory cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Android raw puller temp cleanup sync gate",
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
      /_path_has_surrounding_whitespace_component\([\s\S]*?path[\s\S]*?must not contain surrounding whitespace[\s\S]*?raw output root path must not contain backslashes[\s\S]*?raw output root path may contain backslashes/u,
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
      "--negative-control-android-device-lab-raw-puller-adb-detail-redaction",
      /return NON_UTF8_OUTPUT_REDACTION[\s\S]*?return value\.decode\(\\"utf-8\\", errors=\\"replace\\"\)/u,
      "Android raw puller ADB detail redaction",
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
      "--negative-control-android-device-lab-raw-puller-summary-temp-cleanup-sync-failure",
      /return \[f"\{label\} temporary output cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Android raw puller summary temp cleanup sync gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-published-cleanup-identity",
      /_file_identity\(file_stat\) != expected_identity[\s\S]*?False/u,
      "Android raw puller published cleanup identity gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-published-cleanup-sync-failure",
      /return \[f"\{label\} cleanup could not be synced after parent sync failure"\][\s\S]*?return \[\]/u,
      "Android raw puller published cleanup sync gate",
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
      "--negative-control-android-device-lab-raw-puller-result-chain-digest-guarded-read",
      /hashlib\.sha256\(chain_text\.encode\("utf-8"\)\)\.hexdigest\(\)[\s\S]*?hashlib\.sha256\(chain_file\.read_bytes\(\)\)\.hexdigest\(\)/u,
      "Android raw puller attestation result chain digest guarded-read gate",
    ],
    [
      "--negative-control-android-device-lab-raw-puller-text-read-open-identity",
      /or _file_identity\(path_stat\) != expected_identity[\s\S]*?if _file_identity\(open_stat\) != expected_identity:/u,
      "Android raw puller text read opened-file identity gate",
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
      "--negative-control-android-device-lab-attestation-report-writer-output-early-preflight",
      /output_errors = _preflight_report_output_path\([\s\S]*?output_errors = device_lab\.validate_summary_output_path\(/u,
      "Android attestation report writer output early-preflight gate",
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
      "--negative-control-android-device-lab-attestation-report-writer-published-cleanup-sync-failure",
      /return \[f"\{label\} cleanup could not be synced after parent sync failure"\][\s\S]*?return \[\]/u,
      "Android attestation report writer published cleanup sync gate",
    ],
    [
      "--negative-control-android-device-lab-attestation-report-writer-temp-cleanup-failure",
      /return \[f"\{label\} temporary file could not be removed"\][\s\S]*?return \[\]/u,
      "Android attestation report writer temp cleanup failure gate",
    ],
    [
      "--negative-control-android-device-lab-attestation-report-writer-temp-cleanup-sync-failure",
      /return \[f"\{label\} temporary file cleanup could not be synced"\][\s\S]*?return \[\]/u,
      "Android attestation report writer temp cleanup sync gate",
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
      "--negative-control-release-bundle-android-root-default-wording",
      /preserving the configured default[\s\S]*?preserving the leg[\s\S]*?acy default/u,
      "Kagemusha release bundle Android root configured-default wording",
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
      "--negative-control-release-bundle-verify-existing-evidence-path-shape",
      /blockers\.extend\(_check_release_bundle_evidence_paths\(evidence\)\)[\s\S]*?blockers\.extend\(\[\]\)[\s\S]*?safe_relative in seen_paths[\s\S]*?False and safe_relative in seen_paths/u,
      "Kagemusha release bundle verify-existing evidence path-shape gate",
    ],
    [
      "--negative-control-release-bundle-verify-existing-evidence-digest-uniqueness",
      /seen_digests: set\[str\] = set\(\)[\s\S]*?seen_digests = set\(\)[\s\S]*?digest in seen_digests[\s\S]*?False and digest in seen_digests/u,
      "Kagemusha release bundle verify-existing evidence digest uniqueness gate",
    ],
    [
      "--negative-control-release-bundle-verify-existing-evidence-empty-digest",
      /digest == EMPTY_SHA256_HEX[\s\S]*?False and digest == EMPTY_SHA256_HEX/u,
      "Kagemusha release bundle verify-existing evidence empty-digest gate",
    ],
    [
      "--negative-control-release-bundle-path-component-whitespace",
      /def _path_has_surrounding_whitespace[\s\S]*?path_text != path_text\.strip\(\)[\s\S]*?_path_has_surrounding_whitespace_component[\s\S]*?return False/u,
      "Kagemusha release bundle path component-whitespace preflight",
    ],
    [
      "--negative-control-release-bundle-trusted-signer-path-alias-preflight",
      /must not contain backslashes[\s\S]*?candidate\.parts[\s\S]*?must be a canonical path/u,
      "Kagemusha release bundle trusted signer path-alias preflight",
    ],
    [
      "--negative-control-release-bundle-section-empty-digests",
      /or value == EMPTY_SHA256_HEX[\s\S]*?or False[\s\S]*?or generator_log_sha256 == EMPTY_SHA256_HEX[\s\S]*?or False[\s\S]*?or digest == EMPTY_SHA256_HEX[\s\S]*?or False/u,
      "Kagemusha release bundle section empty-digest gate",
    ],
    [
      "--negative-control-release-bundle-section-digest-distinct",
      /len\(set\(value\.values\(\)\)\) != len\(value\)[\s\S]*?and False/u,
      "Kagemusha release bundle section digest distinctness gate",
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
      "--negative-control-release-bundle-release-evidence-binding",
      /release_evidence_binding_blockers = \([\s\S]*?_check_release_bundle_expected_release_evidence_binding[\s\S]*?release_evidence_binding_blockers = \[\]/u,
      "Kagemusha release bundle nested release evidence binding",
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
      "--negative-control-release-bundle-android-signed-evidence-path-shape",
      /if path == expected_path:[\s\S]*?if True:/u,
      "Kagemusha release bundle Android signed-evidence path shape",
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
      /if value_sha256 in _android_duplicate_binding_slot_values\([\s\S]*?if True:/u,
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
      /existing_signed != expected_signed[\s\S]*?not signed_evidence_binding_without_identity_ok[\s\S]*?if False:/u,
      "Kagemusha release bundle Android signed-evidence summary binding",
    ],
    [
      "--negative-control-release-bundle-android-signed-evidence-binding",
      /entry\.get\("path"\) == expected_entry\.get\("path"\)[\s\S]*?entry\.get\("sha256"\) == expected_entry\.get\("sha256"\)[\s\S]*?entry\.get\("size_bytes"\) == expected_entry\.get\("size_bytes"\)[\s\S]*?if True:/u,
      "Kagemusha release bundle Android signed-evidence entry binding",
    ],
    [
      "--negative-control-release-bundle-android-evidence-inventory-binding",
      /set\(signed_entries\) != set\(expected_signed_entries\)[\s\S]*?False and set\(signed_entries\) != set\(expected_signed_entries\)[\s\S]*?set\(slot_artifacts\) != set\(expected_slot_artifacts\)[\s\S]*?False and set\(slot_artifacts\) != set\(expected_slot_artifacts\)/u,
      "Kagemusha release bundle Android evidence inventory binding",
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
      "--negative-control-release-bundle-manifest-android-slots-binding",
      /kagemusha_release_bundle_manifest_android_slots_binding[\s\S]*?android_manifest_slots_binding_disabled/u,
      "Kagemusha release bundle manifest Android slots binding",
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
      /abi7_archive_fixture_duplicate_archive[\s\S]*?abi7_archive_fixture_name_uniqueness_disabled/u,
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
      "--negative-control-release-bundle-temp-cleanup-sync-failure",
      /"--out temporary file cleanup could not be synced"[\s\S]*?"--out temp cleanup sync is optional"/u,
      "Kagemusha release bundle temp cleanup sync gate",
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

test("Kagemusha staged runner negative controls pin explicit iroha binary validation", () => {
  const readiness = source("ci/check_kagemusha_production_readiness.sh");
  const readinessTests = source("scripts/tests/kagemusha_production_readiness_test.py");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-staged-runner-iroha-bin-validation";

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_production_readiness.sh",
    [mode],
    "Kagemusha staged runner explicit iroha binary validation guard",
  );
  assert.ok(
    readiness.includes(`ci/check_kagemusha_production_readiness.sh ${mode}`),
    `production readiness workflow requirements must include ${mode}`,
  );
  assert.ok(readiness.includes(`if mode == "${mode}":`), `production readiness guard must implement ${mode}`);

  const start = readiness.indexOf(`if mode == "${mode}":`);
  const end = readiness.indexOf("\nif mode ==", start + 1);
  const branch = readiness.slice(start, end === -1 ? readiness.length : end);
  assert.match(branch, /run_negative_control\(/u, "explicit iroha binary validation must use the shared runner");
  assert.match(
    branch,
    /kagemusha_run_lineage_proof_staged\.py[\s\S]*?errors\.extend\(validate_iroha_bin_path\(args\.iroha_bin\)\)[\s\S]*?errors\.extend\(\[\]\)[\s\S]*?kagemusha_run_recursive_compact_keygen_staged\.py[\s\S]*?errors\.extend\(validate_iroha_bin_path\(args\.iroha_bin\)\)[\s\S]*?errors\.extend\(\[\]\)/u,
    "explicit iroha binary negative control must remove both staged-runner validation hooks",
  );

  assertContainsAll(
    readiness,
    [
      "test_compact_key_staged_runner_rejects_unsafe_iroha_bin_before_launch",
      "test_lineage_proof_staged_runner_rejects_unsafe_iroha_bin_before_launch",
      "--iroha-bin must not contain secret-looking material",
      "--iroha-bin must be canonical",
      "--iroha-bin must not contain control characters",
      "--iroha-bin must not contain surrounding whitespace",
      "--iroha-bin must not contain backslashes",
    ],
    "production readiness explicit iroha binary validation inventory",
  );
  assertContainsAll(
    readinessTests,
    [
      "runner must not launch with unsafe --iroha-bin",
      "self.assertFalse(staged_artifact_dir.exists())",
      "self.assertFalse(exit_file.exists())",
      "self.assertFalse(elapsed_file.exists())",
    ],
    "staged runner explicit iroha binary before-launch tests",
  );
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
  assertContainsAll(
    readiness,
    [
      "scripts/kagemusha_staged_resource_guard.py",
      "def run_with_resource_guard(",
      "process.wait(timeout=timeout)",
      "except subprocess.TimeoutExpired:",
      "[kagemusha-staged-runner] {heartbeat_label} heartbeat ",
      "[kagemusha-staged-runner] {heartbeat_label} rss-limit ",
    ],
    "production readiness guard must pin shared staged resource heartbeat loop",
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
      /kagemusha_run_lineage_proof_staged\.py[\s\S]*?STAGED_COMMAND_HEARTBEAT_SECONDS = 300\.0[\s\S]*?STAGED_COMMAND_HEARTBEAT_SECONDS = 0\.0[\s\S]*?kagemusha_staged_resource_guard\.py[\s\S]*?\[kagemusha-staged-runner\] \{heartbeat_label\} heartbeat [\s\S]*?\[kagemusha-staged-runner\] \{heartbeat_label\} quiet /u,
      "lineage staged runner heartbeat",
    ],
    [
      "--negative-control-compact-key-staged-runner-heartbeat",
      /kagemusha_run_recursive_compact_keygen_staged\.py[\s\S]*?STAGED_COMMAND_HEARTBEAT_SECONDS = 300\.0[\s\S]*?STAGED_COMMAND_HEARTBEAT_SECONDS = 0\.0[\s\S]*?kagemusha_staged_resource_guard\.py[\s\S]*?\[kagemusha-staged-runner\] \{heartbeat_label\} heartbeat [\s\S]*?\[kagemusha-staged-runner\] \{heartbeat_label\} quiet /u,
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

test("recursive Kagemusha payload reducer pins size negative controls", () => {
  const reducer = source("ci/check_kagemusha_recursive_spend_payload_bench.sh");
  const policy = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const sizeModes = [
    "--negative-control-payload-baseline",
    "--negative-control-payload-growth",
    "--negative-control-missing-payload",
    "--negative-control-transition-profile-growth",
    "--negative-control-transition-profile-baseline",
    "--negative-control-missing-transition-profile",
    "--negative-control-reserved-lineage-payload-baseline",
    "--negative-control-reserved-lineage-payload-growth",
    "--negative-control-missing-reserved-lineage-payload",
    "--negative-control-reserved-lineage-transition-profile-baseline",
    "--negative-control-reserved-lineage-transition-profile-growth",
    "--negative-control-missing-reserved-lineage-transition-profile",
    "--negative-control-unexpected-payload-hop",
    "--negative-control-unexpected-transition-profile-hop",
    "--negative-control-unexpected-reserved-lineage-payload-hop",
    "--negative-control-unexpected-reserved-lineage-transition-profile-hop",
    "--negative-control-conflicting-payload-size",
    "--negative-control-conflicting-transition-profile-size",
    "--negative-control-conflicting-reserved-lineage-payload-size",
    "--negative-control-conflicting-reserved-lineage-transition-profile-size",
  ];

  assertContainsAll(reducer, sizeModes, "payload reducer size negative controls");
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_payload_bench.sh",
    sizeModes,
    "Kagemusha payload reducer size",
  );
  assertContainsAll(
    policy,
    sizeModes.map((mode) => `ci/check_kagemusha_recursive_spend_payload_bench.sh ${mode}`),
    "policy payload reducer size negative-control commands",
  );

  const explicitBranchModes = [
    "--negative-control-payload-baseline",
    "--negative-control-payload-growth",
    "--negative-control-transition-profile-growth",
    "--negative-control-transition-profile-baseline",
    "--negative-control-reserved-lineage-payload-baseline",
    "--negative-control-reserved-lineage-payload-growth",
    "--negative-control-reserved-lineage-transition-profile-baseline",
    "--negative-control-reserved-lineage-transition-profile-growth",
    "--negative-control-unexpected-payload-hop",
    "--negative-control-unexpected-transition-profile-hop",
    "--negative-control-unexpected-reserved-lineage-payload-hop",
    "--negative-control-unexpected-reserved-lineage-transition-profile-hop",
    "--negative-control-conflicting-payload-size",
    "--negative-control-conflicting-transition-profile-size",
    "--negative-control-conflicting-reserved-lineage-payload-size",
    "--negative-control-conflicting-reserved-lineage-transition-profile-size",
  ];
  for (const mode of explicitBranchModes) {
    assert.ok(reducer.includes(`if mode == "${mode}"`), `payload reducer must implement ${mode}`);
  }
  assertContainsAll(
    reducer,
    [
      'if not (mode == "--negative-control-missing-payload"',
      'if not (mode == "--negative-control-missing-reserved-lineage-payload"',
      'if not (mode == "--negative-control-missing-transition-profile"',
      'if not (mode == "--negative-control-missing-reserved-lineage-transition-profile"',
    ],
    "payload reducer missing-row negative controls",
  );

  const allowedModeBlock = reducer.slice(
    reducer.indexOf("if mode not in {"),
    reducer.indexOf('    "--negative-control-empty-hop-list",'),
  );
  assertContainsAll(allowedModeBlock, sizeModes, "payload reducer size negative-control allow-list");

  const workflowGuard = policy.slice(
    policy.indexOf("def check_workflow_runs_payload_reducer_controls():"),
    policy.indexOf("def check_workflow_runs_policy_negative_controls():"),
  );
  assertContainsAll(
    workflowGuard,
    sizeModes.map((mode) => `ci/check_kagemusha_recursive_spend_payload_bench.sh ${mode}`),
    "policy payload reducer workflow guard",
  );
  assert.match(
    workflowGuard,
    /for label, command in required_before_benchmark:[\s\S]*?workflow_command_match\(workflow, command\)[\s\S]*?Kagemusha payload workflow must run the \{label\} before benchmarking[\s\S]*?Kagemusha payload workflow must run the \{label\} before the real benchmark/u,
    "policy payload reducer workflow guard must require every size control before benchmarking",
  );
});

test("recursive Kagemusha policy workflow and doc negative controls require exact diagnostics", () => {
  const policy = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const branchSpecs = [
    [
      "--negative-control-workflow",
      "Kagemusha payload workflow paths do not cover fail-closed policy sources: javascript/iroha_js/test/kagemushaRecursiveSpend.test.js",
    ],
    [
      "--negative-control-js-package-dist-workflow",
      "Kagemusha payload workflow paths do not cover fail-closed policy sources: javascript/iroha_js/test/package_dist.test.js",
    ],
    [
      "--negative-control-core-isi-workflow",
      "Kagemusha payload workflow paths do not cover fail-closed policy sources: crates/iroha_core/src/smartcontracts/isi/offline.rs",
    ],
    [
      "--negative-control-payload-script-workflow",
      "Kagemusha payload workflow paths do not cover fail-closed policy sources: ci/check_kagemusha_recursive_spend_payload_bench.sh",
    ],
    [
      "--negative-control-ci-guard-script-workflow",
      "Kagemusha payload workflow paths do not cover fail-closed policy sources: ci/check_connect_norito_bridge_header.sh",
    ],
    [
      "--negative-control-payload-self-test-workflow",
      "Kagemusha payload workflow must run the payload reducer self-test before benchmarking",
    ],
    [
      "--negative-control-payload-self-test-order-workflow",
      "Kagemusha payload workflow must run the payload reducer self-test before the real benchmark",
    ],
    [
      "--negative-control-payload-missing-payload-workflow",
      "Kagemusha payload workflow must run the missing payload negative control before benchmarking",
    ],
    [
      "--negative-control-payload-negative-controls-workflow",
      "Kagemusha payload workflow must run the transition-profile growth negative control before benchmarking",
    ],
    [
      "--negative-control-reserved-lineage-payload-negative-controls-workflow",
      "Kagemusha payload workflow must run the Reserved-lineage payload growth negative control before benchmarking",
    ],
    [
      "--negative-control-payload-hop-list-negative-controls-workflow",
      "Kagemusha payload workflow must run the duplicate expected-hop negative control before benchmarking",
    ],
    [
      "--negative-control-payload-benchmark-name-negative-controls-workflow",
      "Kagemusha payload workflow must run the malformed payload benchmark-name negative control before benchmarking",
    ],
    [
      "--negative-control-payload-negative-controls-comment-workflow",
      "Kagemusha payload workflow must run the transition-profile growth negative control before benchmarking",
    ],
    [
      "--negative-control-payload-negative-controls-order-workflow",
      "Kagemusha payload workflow must run the transition-profile growth negative control before the real benchmark",
    ],
    [
      "--negative-control-payload-benchmark-workflow",
      "Kagemusha payload workflow paths do not cover fail-closed policy sources: crates/iroha_data_model/benches/kagemusha_recursive_spend_payload.rs",
    ],
    [
      "--negative-control-payload-benchmark-manifest-workflow",
      "Kagemusha payload workflow paths do not cover fail-closed policy sources: crates/iroha_data_model/Cargo.toml",
    ],
    [
      "--negative-control-doc-payload-budget",
      "docs/source/offline_kagemusha.md is missing previous-proof opening SDK-host boundary documentation: fixed-proof recursive spend bundle at 1,751 bytes",
    ],
    [
      "--negative-control-doc-sdk-host-boundary",
      "docs/source/offline_kagemusha.md is missing previous-proof opening SDK-host boundary documentation: the native bridge and SDK append wrappers validate the metadata tuple",
    ],
    [
      "--negative-control-doc-sdk-availability-surface",
      "docs/source/offline_kagemusha.md is missing previous-proof opening SDK-host boundary documentation: native availability probes: init, append, top-up, both transition-profile helpers, the append-boundary helper, both lineage-witness helpers, verify, and redeem must be callable",
    ],
    [
      "--negative-control-doc-abi-entry-count",
      "docs/source/offline_kagemusha.md contains stale ABI-6 eight-entry wording",
    ],
    [
      "--negative-control-doc-retired-wording",
      "docs/source/offline_kagemusha.md contains stale retired-mode wording: ",
    ],
    [
      "--negative-control-offline-v2-vector-platform-aliases",
      "contains retired Offline V2 vector certificate platform fallback",
    ],
    [
      "--negative-control-offline-vector-platform-aliases",
      "contains retired Offline vector certificate platform fallback",
    ],
    [
      "--negative-control-roadmap-abi-surface",
      "roadmap.md is missing complete recursive spend ABI-6 surface documentation: Bridge ABI 6 adds recursive spend `init`, `append`, both transition-profile helpers, append-boundary derivation, both lineage-witness assembly helpers, `verify`, and `redeem` entry points",
    ],
    [
      "--negative-control-readiness-section-consistency",
      "scripts/kagemusha_production_readiness.py is missing production-readiness section consistency coverage: section_key: _normalized_readiness_section(section_key, section)",
    ],
    [
      "--negative-control-policy-negative-controls-workflow",
      "Kagemusha payload workflow must run the policy core redeem execution-order negative control",
    ],
    [
      "--negative-control-policy-negative-controls-comment-workflow",
      "Kagemusha payload workflow must run the policy core redeem execution-order negative control",
    ],
    [
      "--negative-control-policy-negative-controls-order-workflow",
      "Kagemusha payload workflow must run the policy core redeem execution-order negative control before the main policy guard",
    ],
    [
      "--negative-control-header-negative-controls-workflow",
      "Kagemusha payload workflow must run the NoritoBridge bad recursive header signature negative control",
    ],
    [
      "--negative-control-python-sdk-test-workflow",
      "Kagemusha payload workflow must run the Python recursive spend SDK tests before benchmarking",
    ],
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    branchSpecs.map(([mode]) => mode),
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    policy,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  assert.doesNotMatch(
    policy,
    /^    raise SystemExit\(0\)\n    raise SystemExit\("negative control failed/gmu,
    "policy negative controls must not pass at top level before their failure exit",
  );
  const policyBranch = (mode) => {
    const start = policy.indexOf(`if mode == "${mode}":`);
    assert.notEqual(start, -1, `missing policy branch ${mode}`);
    const end = policy.indexOf("\nif mode ==", start + 1);
    return policy.slice(start, end === -1 ? policy.length : end);
  };

  for (const [mode, expected] of branchSpecs) {
    assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
    const branch = policyBranch(mode);
    const normalizedBranch = branch.replace(/"\s*\n\s*"/gu, "");
    assert.match(
      normalizedBranch,
      new RegExp(
        `${escapeRegExp(expected)}[\\s\\S]*?expected not in message[\\s\\S]*?wrong reason`,
        "u",
      ),
      `${mode} must require its exact diagnostic`,
    );
    assert.doesNotMatch(
      branch,
      /print\(str\(error\)\.splitlines\(\)\[0\]\)/u,
      `${mode} must not print unchecked PolicyError messages`,
    );
  }
  const retiredDocBranch = policyBranch("--negative-control-doc-retired-wording");
  assertContainsAll(
    retiredDocBranch,
    [
      "runtime legacy bearer-audit fallback",
      "pre-existing Halo2 proof-envelope",
      "legacy Halo2 proof-envelope",
      "legacy Offline recursive proof admission",
      "for before, after, label in cases:",
      "documented retired-mode wording drift was not detected for ",
    ],
    "documentation retired-mode wording negative control",
  );
  assert.match(
    retiredDocBranch,
    /expected\s*=\s*\([\s\S]*docs\/source\/offline_kagemusha\.md contains stale retired-mode wording:[\s\S]*\+\s*label/u,
    "documentation retired-mode wording negative control must build exact per-label diagnostics",
  );
  const roadmapAbiBranch = policyBranch("--negative-control-roadmap-abi-surface");
  assertContainsAll(
    roadmapAbiBranch,
    [
      "legacy `OfflineNotePaymentTokenEnvelope` Norito/text handoff",
      "legacy `OfflineNoteReceiveRequestEnvelope` Norito/text handoff",
      "legacy `OfflineNoteReceiptAckEnvelope` Norito/text handoff",
      "classic Torii Offline middleware",
      "roadmap.md contains stale Kagemusha first-release wording: legacy `OfflineNotePaymentTokenEnvelope` Norito/text handoff",
      "roadmap.md contains stale Kagemusha first-release wording: legacy `OfflineNoteReceiveRequestEnvelope` Norito/text handoff",
      "roadmap.md contains stale Kagemusha first-release wording: legacy `OfflineNoteReceiptAckEnvelope` Norito/text handoff",
      "roadmap.md contains stale Kagemusha first-release wording: classic Torii Offline middleware",
    ],
    "roadmap ABI surface negative control must reject C# Offline Note handoff legacy wording",
  );
});

test("recursive Kagemusha active marker scan covers workflow-backed and C# test surfaces", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const activeTodoBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-active-kagemusha-todo":'),
    guard.indexOf('if mode == "--negative-control-active-kagemusha-todo-scan-inventory":'),
  );
  const activeScanInventoryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-active-kagemusha-todo-scan-inventory":'),
    guard.indexOf('if mode == "--negative-control-active-kagemusha-todo-content-scan-inventory":'),
  );
  const activeContentScanInventoryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-active-kagemusha-todo-content-scan-inventory":'),
    guard.indexOf('if mode == "--negative-control-active-kagemusha-todo-runner-input-content-scan-inventory":'),
  );
  const activeRunnerInputContentScanInventoryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-active-kagemusha-todo-runner-input-content-scan-inventory":'),
    guard.indexOf('if mode == "--negative-control-readiness-section-consistency":'),
  );
  const markerName = "TO" + "DO";
  const workflowBackedSurfaces = [
    ".github/workflows/pr_kagemusha_payload_bench.yml",
    "crates/iroha_core/src/tx.rs",
    "crates/iroha_core/src/smartcontracts/isi/offline.rs",
    "crates/iroha_data_model/src/isi/offline.rs",
    "crates/iroha_js_host/src/lib.rs",
    "crates/iroha_torii/src/offline_commands.rs",
    "crates/iroha_torii/src/openapi.rs",
    "crates/iroha_torii/src/zk_prover.rs",
    "crates/iroha_torii/tests/offline_operation_contract.rs",
    "crates/iroha_torii/tests/offline_redeem_contract.rs",
    "kotlin/offline-wallet-android/src/androidTest/java/org/hyperledger/iroha/android/offline/KagemushaDeviceLabArtifactExportTest.java",
    "kotlin/offline-wallet-android/src/androidTest/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
    "crates/iroha_data_model/benches/kagemusha_recursive_spend_payload.rs",
    "scripts/kagemusha_staged_resource_guard.py",
    "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Offline/OfflineNoteWalletNote.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Transactions/KagemushaInstructionArchiveInstruction.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/OfflineNoteWalletNoteTests.cs",
  ];
  const genericContentSurfaces = [
    "Cargo.toml",
    "javascript/iroha_js/scripts/build-native.mjs",
    "javascript/iroha_js/scripts/copy-native.mjs",
    "javascript/iroha_js/src/native.js",
    "javascript/iroha_js/dist/native.js",
    "python/norito_py/src/norito/codec.py",
    "java/norito_java/src/main/java/org/hyperledger/iroha/norito/NoritoCodec.java",
    "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
    "IrohaSwift/Sources/IrohaSwift/ToriiClient.swift",
    "IrohaSwift/Sources/IrohaSwift/TransactionEncoder.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift",
    "crates/iroha_cli/src/main_shared.rs",
    "crates/iroha_core/src/executor.rs",
    "crates/iroha_core/src/queue/router.rs",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/IrohaOfflineNoteTransactionSubmitter.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNotePaymentTokenCodec.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteWallet.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineNoteWalletNoteJsonCodec.java",
    "javascript/iroha_js/src/toriiClient.js",
    "javascript/iroha_js/test/integrationTorii.test.js",
    "javascript/iroha_js/test/privacyCatalogParity.test.js",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineNoteWallet.kt",
    "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift",
    "python/iroha_python/src/iroha_python/offline_cash.py",
    "csharp/README.md",
    "csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionBuilder.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionInstruction.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Zk/VerifyingKeyBackendTag.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/OfflineCashLifecycleTests.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/TransactionBuilderTests.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/VerifyingKeyBackendTagTests.cs",
  ];

  assertContainsAll(
    guard,
    workflowBackedSurfaces,
    "active Kagemusha marker scan must cover workflow-backed, Android, and C# test surfaces",
  );
  assertContainsAll(
    activeTodoBranch,
    [
      ...workflowBackedSurfaces,
      "core offline ISI active marker",
      "payload workflow active marker",
      "Torii Kagemusha-only smoke active marker",
      "Torii offline-v2 smoke active marker",
      "Android device-lab instrumentation active marker",
      "Android recursive spend instrumentation active marker",
      "payload benchmark active marker",
      "staged resource guard script active marker",
      "C# recursive spend active marker",
      "C# wallet-note strict-state active marker",
      "C# wallet-note strict-state test active marker",
      "csharp/src/Hyperledger.Iroha.Sdk/Offline/KagemushaRecursiveSpend.cs",
      "csharp/src/Hyperledger.Iroha.Sdk/Offline/OfflineNoteWalletNote.cs",
      "csharp/tests/Hyperledger.Iroha.Sdk.Tests/OfflineNoteWalletNoteTests.cs",
      "generic content Python SDK active marker",
      "python/iroha_python/src/iroha_python/offline_cash.py",
      "roadmap late C# handoff scope",
    ],
    "active Kagemusha marker negative control must mutate each workflow-backed and C# test surface",
  );
  assert.ok(
    new RegExp(
      `f"\\{todo_prefix\\} bypass Kagemusha C# SDK matrix native-output certification\\\\n"[\\s\\S]*?The Ubuntu/Windows C# SDK matrix must keep matching exact C# native-output`,
      "u",
    ).test(activeTodoBranch),
    "active marker negative control must mutate a roadmap matrix requirement",
  );
  assert.ok(
    !activeTodoBranch.includes(
      `${markerName}: bypass Kagemusha C# SDK matrix native-output certification`,
    ),
    "active marker negative control must keep active marker text split in source",
  );
  assert.doesNotMatch(
    guard,
    new RegExp(`ROADMAP_ACTIVE_KAGEMUSHA_${markerName}_SCAN_LINE_LIMIT`, "u"),
    "roadmap active marker scan must cover the full file",
  );
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    [
      "--negative-control-active-kagemusha-todo",
      "--negative-control-active-kagemusha-todo-scan-inventory",
      "--negative-control-active-kagemusha-todo-content-scan-inventory",
      "--negative-control-active-kagemusha-todo-runner-input-content-scan-inventory",
    ],
    "Kagemusha policy guard",
  );
  assert.match(
    activeTodoBranch,
    /for target, before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "active Kagemusha marker negative control must validate every mutated surface independently",
  );
  assertContainsAll(
    guard,
    [
      `ACTIVE_KAGEMUSHA_${markerName}_SCAN_DISCOVERY_ROOTS`,
      `ACTIVE_KAGEMUSHA_${markerName}_SCAN_DISCOVERY_ALLOWLIST`,
      `REQUIRED_KAGEMUSHA_RUNNER_INPUT_${markerName}_CONTENT_SCAN_PATHS`,
      `ACTIVE_KAGEMUSHA_${markerName}_CONTENT_SCAN_PATHS`,
      `ACTIVE_KAGEMUSHA_${markerName}_CONTENT_SCAN_DISCOVERY_ROOTS`,
      "discover_active_kagemusha_todo_scan_paths",
      "discover_active_kagemusha_todo_content_scan_paths",
      `${markerName}_MARKER_RE.search(line) and KAGEMUSHA_CONTENT_RE.search(line)`,
      "ci/check_kagemusha_recursive_spend_policy.sh",
      "ci/check_kagemusha_recursive_spend_csharp_sdk.sh",
      '"csharp"',
    ],
    "active marker scan inventory must discover source-like and content-bearing Kagemusha paths",
  );
  assertContainsAll(
    guard,
    genericContentSurfaces,
    "active content marker scan must cover generic source files that mention Kagemusha",
  );
  assert.match(
    guard,
    new RegExp(
      `active Kagemusha ${markerName} scan does not cover source-like Kagemusha path\\(s\\):`,
      "u",
    ),
    "active marker scan inventory must fail on unscanned source-like Kagemusha paths",
  );
  assert.match(
    guard,
    new RegExp(
      `active Kagemusha ${markerName} content scan does not cover content-bearing source path\\(s\\):`,
      "u",
    ),
    "active marker content-scan inventory must fail on unscanned content-bearing Kagemusha paths",
  );
  assert.match(
    guard,
    new RegExp(
      `active Kagemusha ${markerName} content scan does not cover runner input path\\(s\\):`,
      "u",
    ),
    "active marker content-scan inventory must fail on unscanned runner input paths",
  );
  assertContainsAll(
    activeScanInventoryBranch,
    [
      "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendV2Tests.swift",
      `ACTIVE_KAGEMUSHA_${markerName}_SCAN_PATHS = tuple(`,
      "run_checks()",
      `active Kagemusha ${markerName} scan inventory drift was not detected`,
    ],
    "active marker scan inventory negative control must remove a discovered source path",
  );
  assertContainsAll(
    activeContentScanInventoryBranch,
    [
      "csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionBuilder.cs",
      `ACTIVE_KAGEMUSHA_${markerName}_CONTENT_SCAN_PATHS = tuple(`,
      "run_checks()",
      `active Kagemusha ${markerName} content scan inventory drift was not detected`,
    ],
    "active marker content-scan inventory negative control must remove a discovered generic content path",
  );
  assertContainsAll(
    activeRunnerInputContentScanInventoryBranch,
    [
      "javascript/iroha_js/scripts/build-native.mjs",
      `ACTIVE_KAGEMUSHA_${markerName}_CONTENT_SCAN_PATHS = tuple(`,
      "run_checks()",
      `active Kagemusha ${markerName} runner-input content scan inventory drift was not detected`,
      "runner input path(s): {target}",
    ],
    "active marker runner-input content-scan inventory negative control must remove a required runner input path",
  );
});

test("recursive Kagemusha policy tail negative controls require exact diagnostics", () => {
  const policy = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const branchSpecs = [
    [
      "--negative-control-core-isi",
      "{target} is missing Reserved-lineage adversarial coverage: fn kagemusha_recursive_redeem_rejects_semantic_recursive_spend_before_mint",
    ],
    [
      "--negative-control-core-multi-hop-redeem-success",
      "{target} is missing Reserved-lineage adversarial coverage: fn kagemusha_recursive_redeem_record_backed_multi_hop_mints_and_rejects_replay",
    ],
    [
      "--negative-control-core-lineage-hop-proof",
      "{target} is missing Reserved-lineage adversarial coverage: fn kagemusha_recursive_redeem_rejects_malformed_lineage_hop_proof_before_mint",
    ],
    [
      "--negative-control-core-redeem-order",
      "recursive Kagemusha redeem execution order no longer gates mint behind lineage, proof, and nullifier checks: missing state_transaction.register_confidential_proof(self.redeem_proof.proof.bytes.len())",
    ],
    [
      "--negative-control-core-redeem-early-mint",
      "recursive Kagemusha redeem must have exactly one production mint construction after all lineage, proof, and nullifier gates; found 2",
    ],
    [
      "--negative-control-status-doc-drift",
      "status.md contains stale one-hop witnessless chain-redemption boundary",
    ],
    [
      "--negative-control-workflow-cancel-in-progress",
      "Kagemusha payload workflow must not cancel in-progress runs; long proof/benchmark evidence must be allowed to finish",
    ],
    [
      "--negative-control-main-guards-workflow",
      "Kagemusha payload workflow must run the main Kagemusha recursive spend SDK parity guard",
    ],
  ];

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    branchSpecs.map(([mode]) => mode),
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    policy,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );
  const policyBranch = (mode) => {
    const start = policy.indexOf(`if mode == "${mode}":`);
    assert.notEqual(start, -1, `missing policy branch ${mode}`);
    const end = policy.indexOf("\nif mode ==", start + 1);
    return policy.slice(start, end === -1 ? policy.length : end);
  };

  for (const [mode, expected] of branchSpecs) {
    assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
    const normalizedBranch = policyBranch(mode).replace(/"\s*\n\s*"/gu, "");
    assert.match(
      normalizedBranch,
      new RegExp(
        `${escapeRegExp(expected)}[\\s\\S]*?expected not in message[\\s\\S]*?wrong reason`,
        "u",
      ),
      `${mode} must require its exact diagnostic`,
    );
    assert.doesNotMatch(
      normalizedBranch,
      /print\(str\(error\)\.splitlines\(\)\[0\]\)/u,
      `${mode} must not print unchecked PolicyError messages`,
    );
  }
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
      "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProverTest.kt",
      "KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 to Int.MAX_VALUE",
      "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java",
      "KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, Integer.MAX_VALUE",
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

test("recursive Kagemusha policy edge-case negative controls require exact missing labels", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const specs = [
    [
      "--negative-control-sdk-selector-edge",
      "--negative-control-sdk-preferred-cap-edge",
      "semantic previous proofs cannot select Reserved-lineage output",
      "SDK selector edge drift was not detected",
    ],
    [
      "--negative-control-sdk-preferred-cap-edge",
      "--negative-control-js-package-dist-selector-edge",
      "the semantic append circuit remains preferred while lineage transition verification is unavailable",
      "SDK preferred cap edge drift was not detected",
    ],
    [
      "--negative-control-js-package-dist-selector-edge",
      "--negative-control-python-hop-edges",
      "semantic previous proofs cannot select Reserved-lineage output",
      "JavaScript package-dist selector edge drift was not detected",
    ],
    [
      "--negative-control-python-hop-edges",
      "--negative-control-js-hop-edges",
      'float("nan")',
      "Python hop-count policy drift was not detected",
    ],
    [
      "--negative-control-js-hop-edges",
      "--negative-control-js-package-dist-hop-edges",
      "canAppendKagemushaRecursiveSpendWitnesslessLineage(1n)",
      "JavaScript BigInt hop-count policy drift was not detected",
    ],
    [
      "--negative-control-js-package-dist-hop-edges",
      "--negative-control-sdk-append-cap-binding",
      "canAppendKagemushaRecursiveSpendWitnesslessLineage(1n)",
      "JavaScript package-dist BigInt hop-count policy drift was not detected",
    ],
  ];
  const modes = specs.map(([mode]) => mode);

  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    modes,
    "Kagemusha policy guard",
  );
  const inventoryModes = negativeControlModesFromInventory(
    guard,
    "POLICY_NEGATIVE_CONTROL_COMMANDS = (",
    "class PolicyError",
  );

  for (const [mode, nextMode, label, failure] of specs) {
    assert.ok(inventoryModes.includes(mode), `policy negative-control inventory must include ${mode}`);
    const start = guard.indexOf(`if mode == "${mode}":`);
    const end = guard.indexOf(`if mode == "${nextMode}":`, start + 1);
    assert.notEqual(start, -1, `policy guard must implement ${mode}`);
    assert.notEqual(end, -1, `policy guard must terminate ${mode} before ${nextMode}`);
    const branch = guard.slice(start, end);
    assert.ok(branch.includes(label), `${mode} must pin exact missing label ${label}`);
    const doubleQuotedLabel = escapeRegExp(JSON.stringify(label));
    const singleQuotedLabel = escapeRegExp(`'${label}'`);
    assert.match(
      branch,
      new RegExp(`if\\s+(?:${doubleQuotedLabel}|${singleQuotedLabel})\\s+not\\s+in\\s+message:`, "u"),
      `${mode} must check the exact missing label in the PolicyError message`,
    );
    assert.match(
      branch,
      /except\s+PolicyError\s+as\s+error:[\s\S]*?message = str\(error\)[\s\S]*?if [\s\S]*? not in message:[\s\S]*?raise SystemExit\("negative control failed:[\s\S]*?print\(message\.splitlines\(\)\[0\]\)[\s\S]*?raise SystemExit\(0\)/u,
      `${mode} must only pass after checking the exact missing-label message`,
    );
    assert.ok(
      branch.includes(`raise SystemExit("negative control failed: ${failure}")`),
      `${mode} must fail when exact drift is not detected`,
    );
    assert.doesNotMatch(
      branch,
      /print\(str\(error\)\.splitlines\(\)\[0\]\)/u,
      `${mode} must not print unchecked PolicyError messages`,
    );
  }
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
    guard.indexOf('if mode == "--negative-control-js-host-append-boundary-current-output-set":'),
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
    /expected = \([\s\S]*?\{target\} is missing Reserved-lineage adversarial coverage:[\s\S]*?KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES \+ 1[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
    "JS host archive-cap negative control must require the exact cap-plus-one diagnostic",
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
    guard.indexOf('if mode == "--negative-control-python-append-boundary-current-output-set":'),
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
    /expected = \([\s\S]*?\{target\} is missing Reserved-lineage adversarial coverage:[\s\S]*?KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES \+ 1[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
    "Python host archive-cap negative control must require the exact cap-plus-one diagnostic",
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
      "export const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 256 * 1024 * 1024;",
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

  const sharedFixtureModes = [
    "--negative-control-shared-fixture-manifest",
    "--negative-control-shared-archive-fixture",
    "--negative-control-request-archive-required-fields",
    "--negative-control-first-release-archive-required-fields",
    "--negative-control-shared-abi7-fixture-manifest",
    "--negative-control-shared-abi7-archive-fixture",
  ];
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_policy.sh",
    sharedFixtureModes,
    "Kagemusha policy guard",
  );
  for (const sharedMode of sharedFixtureModes) {
    assert.ok(
      inventoryModes.includes(sharedMode),
      `policy negative-control inventory must include ${sharedMode}`,
    );
  }
  const abi6ManifestBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-shared-fixture-manifest":'),
    guard.indexOf('if mode == "--negative-control-shared-archive-fixture":'),
  );
  assert.match(
    abi6ManifestBranch,
    /"operation_count": 9[\s\S]*?"operation_count": 8[\s\S]*?expected = f"\{target\} must contain exactly nine ABI-6 operations"[\s\S]*?expected not in message/u,
    "ABI-6 fixture manifest negative control must require the exact operation-count diagnostic",
  );
  const abi6ArchiveBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-shared-archive-fixture":'),
    guard.indexOf('if mode == "--negative-control-request-archive-required-fields":'),
  );
  assert.match(
    abi6ArchiveBranch,
    /c5402b3ea6aeb35ce12607344304b858273f8589e2b3887708a86cb19665ce68[\s\S]*?00402b3ea6aeb35ce12607344304b858273f8589e2b3887708a86cb19665ce68[\s\S]*?is missing shared recursive spend ABI-6 fixture coverage[\s\S]*?expected not in message/u,
    "ABI-6 archive fixture negative control must require the exact archive hash diagnostic",
  );
  const requestArchiveRequiredFieldsBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-request-archive-required-fields":'),
    guard.indexOf('if mode == "--negative-control-first-release-archive-required-fields":'),
  );
  assertContainsAll(
    requestArchiveRequiredFieldsBranch,
    [
      "crates/iroha_data_model/src/offline/mod.rs",
      "#[norito(default)]\\n        pub lineage_verifier_key: Option<VerifyingKeyBox>,",
      "recursive spend request/result archive fields must not use #[norito(default)]",
      "SHARED_ARCHIVE_FIXTURE_PATH",
      '"norito_default": false',
      '"norito_default": true',
      "KagemushaRecursiveSpendInitRequestV1.lineage_verifier_key must advertise norito_default false",
      "request archive required-fields drift was not detected for ",
    ],
    "request archive required-fields negative control",
  );
  assert.match(
    requestArchiveRequiredFieldsBranch,
    /for target, before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "request archive required-fields negative control must validate each mutated text snapshot",
  );
  assert.match(
    requestArchiveRequiredFieldsBranch,
    /if label not in message:[\s\S]*?request archive required-fields drift was rejected for the wrong reason[\s\S]*?if first_message is None:[\s\S]*?raise SystemExit\("negative control failed: request archive required-fields drift was not detected"\)[\s\S]*?raise SystemExit\(0\)/u,
    "request archive required-fields negative control must only pass after exact diagnostics",
  );
  const firstReleaseArchiveRequiredFieldsBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-first-release-archive-required-fields":'),
    guard.indexOf('if mode == "--negative-control-shared-abi7-fixture-manifest":'),
  );
  assertContainsAll(
    firstReleaseArchiveRequiredFieldsBranch,
    [
      "crates/iroha_data_model/src/offline/mod.rs",
      "#[norito(default)]\\n        pub transition_profile_binding_digest: [u8; 32],",
      "KagemushaRecursiveAggregationProofPublicInputs first-release recursive spend archive fields must not use #[norito(default)]",
      "first-release archive required-fields drift was not detected",
    ],
    "first-release archive required-fields negative control",
  );
  assert.match(
    firstReleaseArchiveRequiredFieldsBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)/u,
    "first-release archive required-fields negative control must only pass after exact diagnostics",
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
    /"operation_count": 5[\s\S]*?"operation_count": 4[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?expected = f"\{target\} must contain exactly five ABI-7 fixture operations"[\s\S]*?expected not in message/u,
    "ABI-7 fixture manifest negative control must mutate the manifest and require the exact operation-count diagnostic",
  );
  const abi7ArchiveBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-shared-abi7-archive-fixture":'),
    guard.indexOf('if mode == "--negative-control-shared-abi7-sdk-manifest-coverage":'),
  );
  assert.match(
    abi7ArchiveBranch,
    /42c7b1b0e2dc838a6660b3691e08474bb936fa001e446310930d387b00ba686b[\s\S]*?00c7b1b0e2dc838a6660b3691e08474bb936fa001e446310930d387b00ba686b[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?is missing shared recursive spend ABI-7 fixture coverage[\s\S]*?expected not in message/u,
    "ABI-7 archive fixture negative control must mutate the archive and require the exact hash diagnostic",
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
      "IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendV2Tests.swift",
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
    "--negative-control-core-append-cap-boundary",
    "--negative-control-core-lineage-profile-split",
    "--negative-control-roadmap-current-profile-staleness",
    "--negative-control-roadmap-semantic-init-lineage-key-wording",
    "--negative-control-core-proof-chain-accumulator",
    "--negative-control-core-fixed-window-table-base-accumulator",
    "--negative-control-core-shared-table-identity-base-selection",
    "--negative-control-core-shared-table-direct-mode-duplicate-witnesses",
    "--negative-control-core-shared-table-witness-shape-guards",
    "--negative-control-core-shared-table-direct-base-helper",
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
    "--negative-control-data-model-append-cap-boundary",
    "--negative-control-data-model-self-consistent-boundary",
    "--negative-control-data-model-zero-prehash-hash-guard",
    "--negative-control-data-model-transition-profile-current-hop-sets",
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
      "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-255x1",
      "pub const KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOWS: usize = 255;",
      "pub const KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOW_BITS: usize = 1;",
      "CURRENT_ROADMAP_PROFILE_NEEDLES",
      "STALE_ROADMAP_PROFILE_MARKERS",
      "CURRENT_ROADMAP_SEMANTIC_INIT_NEEDLES",
      "STALE_ROADMAP_SEMANTIC_INIT_MARKERS",
      "roadmap.md still references stale Kagemusha verifier-witness profile marker",
      "roadmap.md contains stale Kagemusha semantic-init lineage-key wording",
    ],
    "Kagemusha policy verifier witness profile source pins",
  );

  const dataModelAppendCapBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-append-cap-boundary":'),
    guard.indexOf('if mode == "--negative-control-data-model-self-consistent-boundary":'),
  );
  assert.match(
    dataModelAppendCapBranch,
    /Reserved-lineage append request at the witnessless hop cap must reject before proving[\s\S]*?Reserved-lineage append request at the hop edge[\s\S]*?is missing Reserved-lineage adversarial coverage[\s\S]*?expected not in message/u,
    "data-model append cap negative control must require the exact adversarial coverage diagnostic",
  );

  const selfConsistentBoundaryBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-self-consistent-boundary":'),
    guard.indexOf('if mode == "--negative-control-data-model-zero-prehash-hash-guard":'),
  );
  assertContainsAll(
    selfConsistentBoundaryBranch,
    [
      "cases = (",
      "zero-prehash recursive compact digest must be rejected",
      "zero-prehash recursive compact digest may pass",
      "zero-prehash checked-prefold compact digest must be rejected",
      "zero-prehash checked-prefold compact digest may pass",
      "zero_accumulator_nullifier_digest.nullifier_digest =",
      "zero_accumulator_nullifier_digest.nullifier_digest_unchecked =",
      "zero_previous_accumulator_pi.previous_accumulator_public_inputs_hash =",
      "zero_previous_accumulator_pi.previous_accumulator_public_inputs_hash_unchecked =",
      "zero_profile_resulting_public_inputs_hash.resulting_public_inputs_hash =",
      "zero_profile_resulting_public_inputs_hash.resulting_public_inputs_hash_unchecked =",
      "fn assert_self_consistent_forged_boundary_rejected(",
      "fn assert_profile_bound_forged_boundary_rejected(",
      "zero_append_opening_preflight_current_hop_proof_hash.current_hop_proof_hash =",
      "zero_append_opening_preflight_current_hop_proof_hash.current_hop_proof_hash_unchecked =",
      "zero_current_hop_proof_hash.current_hop_proof_hash =",
      "zero_current_hop_proof_hash.current_hop_proof_hash_unchecked =",
      "zero_resulting_public_inputs_hash.resulting_public_inputs_hash =",
      "zero_resulting_public_inputs_hash.resulting_public_inputs_hash_unchecked =",
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
  assertContainsAll(
    guard,
    [
      "ZeroProofHash {",
      "zero_proof_hash.steps[0].proof_hash = Hash::prehashed([0u8; Hash::LENGTH]);",
      "zero_proof_hash.proof_hash = Hash::prehashed([0u8; Hash::LENGTH]);",
      "assert_zero_checked_prefold_compact_hash_rejected(",
      "zero-prehash checked-prefold compact digest must be rejected",
      "assert_zero_recursive_compact_hash_rejected(",
      "zero-prehash recursive compact digest must be rejected",
    ],
    "data-model policy coverage must pin folded-input zero-prehash adversarial assertions",
  );

  const zeroPrehashHashGuardBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-zero-prehash-hash-guard":'),
    guard.indexOf('if mode == "--negative-control-data-model-transition-profile-current-hop-sets":'),
  );
  assertContainsAll(
    guard,
    [
      "def check_kagemusha_hash_zero_sentinel_guards():",
      "fn is_zero_prehash_hash(hash: Hash) -> bool",
      "hash == Hash::prehashed([0u8; Hash::LENGTH])",
      "is_zero_prehash_hash(preflight.current_hop_proof_hash)",
      "is_zero_prehash_hash(boundary.current_hop_proof_hash)",
      "is_zero_prehash_hash(boundary.resulting_public_inputs_hash)",
      "fold-step proof-hash zero-prehash validation",
      "proof_hash: Hash",
      "if is_zero_prehash_hash(proof_hash)",
      "hash_bytes_from_hash(proof_hash)",
      "fold-step proof-hash zero-prehash validation must not convert Hash digests to bytes before zero checks",
      "checked-prefold compact zero-prehash Hash validation",
      "recursive compact zero-prehash Hash validation",
      '("nullifier_digest", self.nullifier_digest)',
      '("output_commitment_digest", self.output_commitment_digest)',
      '("fold_digest", self.fold_digest)',
      "hash_bytes_from_hash(self.nullifier_digest)",
      "checked-prefold compact zero-prehash Hash validation must not convert Hash digests to bytes before zero checks",
      "recursive compact zero-prehash Hash validation must not convert Hash digests to bytes before zero checks",
      "Kagemusha Hash zero-sentinel guard must not compare Hash bytes to all-zero arrays",
    ],
    "data-model zero-prehash Hash guard must pin sentinel helper and diagnostic",
  );
  assert.match(
    zeroPrehashHashGuardBranch,
    /is_zero_prehash_hash\(boundary\.current_hop_proof_hash\)[\s\S]*?hash_bytes_from_hash\(boundary\.current_hop_proof_hash\) == \[0u8; Hash::LENGTH\]/u,
    "zero-prehash Hash guard negative control must reintroduce the raw-byte zero comparison",
  );
  assert.match(
    zeroPrehashHashGuardBranch,
    /is_zero_prehash_hash\(proof_hash\)[\s\S]*?hash_bytes_from_hash\(proof_hash\) == \[0u8; Hash::LENGTH\]/u,
    "zero-prehash Hash guard negative control must reintroduce the proof-hash raw-byte zero comparison",
  );
  assert.match(
    zeroPrehashHashGuardBranch,
    /"if is_zero_prehash_hash\(digest\) \{\\n\s*return Err\(KagemushaFoldError::ZeroFoldedPublicInputDigest \{ field \}\);",[\s\S]*?"if hash_bytes_from_hash\(digest\) == \[0u8; Hash::LENGTH\] \{\\n\s*return Err\(KagemushaFoldError::ZeroFoldedPublicInputDigest \{ field \}\);"/u,
    "zero-prehash Hash guard negative control must reintroduce a folded-input raw-byte zero comparison",
  );
  assert.match(
    zeroPrehashHashGuardBranch,
    /for before, after, expected in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "zero-prehash Hash guard negative control must validate each mutated text snapshot",
  );
  assert.match(
    zeroPrehashHashGuardBranch,
    /if expected not in message:[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: zero-prehash Hash guard drift was not detected"\)/u,
    "zero-prehash Hash guard negative control must require the exact diagnostic",
  );
  assert.match(
    zeroPrehashHashGuardBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?if first_message is None:[\s\S]*?continue[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: zero-prehash Hash guard drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "zero-prehash Hash guard negative control must only pass after every injected drift is detected",
  );
  assert.doesNotMatch(
    zeroPrehashHashGuardBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "zero-prehash Hash guard negative control must not unconditionally pass after run_checks",
  );

  const transitionProfileCurrentHopSetsBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-transition-profile-current-hop-sets":'),
    guard.indexOf('if mode == "--negative-control-data-model-transition-profile-previous-topup-anchors":'),
  );
  assertContainsAll(
    transitionProfileCurrentHopSetsBranch,
    [
      "validate_kagemusha_unique_input_output_sets",
      "validate_kagemusha_unique_input_output_sets(\\n        hop_index_usize,",
      "duplicate_initial_input_profile",
      "Err(KagemushaFoldError::DuplicateInputNullifier { hop_index: 0 })",
      "overlapping_initial_output_profile",
      "Err(KagemushaFoldError::InputOutputOverlap { hop_index: 0 })",
      "duplicate_append_output_profile",
      "Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 1 })",
    ],
    "transition-profile current-hop set negative control must mutate helper, call site, and every adversarial assertion",
  );
  assert.match(
    transitionProfileCurrentHopSetsBranch,
    /for before, after in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "transition-profile current-hop set negative control must validate each mutated text snapshot",
  );
  assert.match(
    transitionProfileCurrentHopSetsBranch,
    /expected = \([\s\S]*?is missing Reserved-lineage adversarial coverage:[\s\S]*?\+ before[\s\S]*?if expected not in message:/u,
    "transition-profile current-hop set negative control must require the exact adversarial coverage diagnostic",
  );
  assert.match(
    transitionProfileCurrentHopSetsBranch,
    /transition-profile current-hop set drift was not detected for[\s\S]*?print\("negative control rejected transition-profile current-hop set drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "transition-profile current-hop set negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    transitionProfileCurrentHopSetsBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "transition-profile current-hop set negative control must not unconditionally pass after run_checks",
  );

  const transitionProfilePreviousTopupAnchorsBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-transition-profile-previous-topup-anchors":'),
    guard.indexOf('if mode == "--negative-control-data-model-transition-profile-resulting-accumulator":'),
  );
  assertContainsAll(
    transitionProfilePreviousTopupAnchorsBranch,
    [
      "pub previous_topup_anchor_nullifiers: Vec<[u8; 32]>",
      ".map(|previous| previous.topup_anchor_nullifiers.clone())",
      "validate_kagemusha_recursive_spend_topup_anchor_nullifiers_field(",
      ".any(|commitment| self.previous_topup_anchor_nullifiers.contains(commitment))",
      "topup_anchor_as_append_output",
      "topup_anchor_as_current_nullifier",
    ],
    "transition-profile previous top-up negative control must mutate every carryover, source guard, and adversarial test marker",
  );
  assert.match(
    transitionProfilePreviousTopupAnchorsBranch,
    /for before, after in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "transition-profile previous top-up negative control must validate each mutated text snapshot",
  );
  assert.match(
    transitionProfilePreviousTopupAnchorsBranch,
    /expected = \([\s\S]*?is missing Reserved-lineage adversarial coverage:[\s\S]*?\+ before[\s\S]*?if expected not in message:/u,
    "transition-profile previous top-up negative control must require the exact missing marker diagnostic",
  );
  assert.match(
    transitionProfilePreviousTopupAnchorsBranch,
    /transition-profile previous top-up anchor drift was not detected for[\s\S]*?print\("negative control rejected transition-profile previous top-up anchor drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "transition-profile previous top-up negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    transitionProfilePreviousTopupAnchorsBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "transition-profile previous top-up negative control must not unconditionally pass after run_checks",
  );

  const transitionProfileResultingAccumulatorBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-transition-profile-resulting-accumulator":'),
    guard.indexOf('if mode == "--negative-control-data-model-proof-public-input-circuit-binding":'),
  );
  assertContainsAll(
    transitionProfileResultingAccumulatorBranch,
    [
      "fn validate_kagemusha_recursive_spend_transition_profile_resulting_accumulator(",
      "validate_kagemusha_recursive_spend_transition_profile_resulting_accumulator(self)?;",
      "resulting_accumulator.transition_profile_binding_digest = transition_profile_binding_digest;",
      "profile.resulting_accumulator_digest =\\n        kagemusha_recursive_spend_accumulator_digest(&resulting_accumulator)?;",
      "profile.resulting_public_inputs_hash =\\n        kagemusha_recursive_spend_append_boundary_free_public_inputs_hash(&resulting_accumulator)?;",
      "kagemusha_recursive_spend_transition_profile_binding_digest_unchecked(profile)?",
      "profile.resulting_accumulator_digest\\n        != kagemusha_recursive_spend_accumulator_digest(&expected_accumulator)?",
      "profile.resulting_public_inputs_hash\\n        != kagemusha_recursive_spend_append_boundary_free_public_inputs_hash(&expected_accumulator)?",
      "forged_resulting_accumulator_digest.validate_context()",
      "forged_resulting_public_inputs_hash.validate_context()",
    ],
    "transition-profile resulting accumulator negative control must mutate construction, validation, and adversarial markers",
  );
  assert.match(
    transitionProfileResultingAccumulatorBranch,
    /for before, after in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "transition-profile resulting accumulator negative control must validate each mutated text snapshot",
  );
  assert.match(
    transitionProfileResultingAccumulatorBranch,
    /expected = \([\s\S]*?is missing Reserved-lineage adversarial coverage:[\s\S]*?\+ before[\s\S]*?if expected not in message:/u,
    "transition-profile resulting accumulator negative control must require the exact missing marker diagnostic",
  );
  assert.match(
    transitionProfileResultingAccumulatorBranch,
    /transition-profile resulting accumulator drift was not detected for[\s\S]*?print\("negative control rejected transition-profile resulting accumulator drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "transition-profile resulting accumulator negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    transitionProfileResultingAccumulatorBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "transition-profile resulting accumulator negative control must not unconditionally pass after run_checks",
  );

  const proofPublicInputCircuitBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-proof-public-input-circuit-binding":'),
    guard.indexOf('if mode == "--negative-control-data-model-previous-proof-field-binding":'),
  );
  assertContainsAll(
    proofPublicInputCircuitBranch,
    [
      "if accumulator.append_boundary_digest != [0u8; Hash::LENGTH]",
      "if recursive_proof.public_inputs.append_boundary_digest != [0u8; Hash::LENGTH]",
      "if accumulator.append_opening_preflight_digest != [0u8; Hash::LENGTH]",
      "if recursive_proof\\n                .public_inputs\\n                .append_opening_preflight_digest",
      "let scalar_projection = recursive_proof\\n                .public_inputs\\n                .recursive_verifier_scalar_projection_digest;",
      "if scalar_projection == [0u8; Hash::LENGTH]",
      "expected.recursive_verifier_scalar_projection_digest = scalar_projection;",
      "let append_boundary_digest = recursive_proof.public_inputs.append_boundary_digest;",
      "if expected.append_opening_preflight_digest == [0u8; Hash::LENGTH]",
      "if append_boundary_digest != [0u8; Hash::LENGTH]",
      "if expected.append_boundary_digest == [0u8; Hash::LENGTH]",
      "if append_boundary_digest != expected.append_boundary_digest",
    ],
    "proof public-input circuit binding negative control must mutate semantic and lineage proof-routing markers",
  );
  assert.match(
    proofPublicInputCircuitBranch,
    /for before, after, expected_marker in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "proof public-input circuit binding negative control must validate each mutated text snapshot",
  );
  assert.match(
    proofPublicInputCircuitBranch,
    /expected = \([\s\S]*?recursive spend proof public-input circuit binding is missing ordered preverification step:[\s\S]*?\+ expected_marker[\s\S]*?if expected not in message:/u,
    "proof public-input circuit binding negative control must require the exact missing ordered-step diagnostic",
  );
  assert.match(
    proofPublicInputCircuitBranch,
    /proof public-input circuit binding drift was not detected for[\s\S]*?print\("negative control rejected proof public-input circuit binding drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "proof public-input circuit binding negative control must only pass after all injected drift is detected",
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
    /recursive spend proof public-input circuit binding is missing ordered preverification step:[\s\S]*?if accumulator\.append_opening_preflight_digest != \[0u8; Hash::LENGTH\][\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: semantic proof append-opening drift was not detected"\)/u,
    "semantic proof append-opening negative control must only pass after detecting the exact ordered-step drift",
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
    /missing Rust one-hop append-opening public-input rejection[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: one-hop append-opening public-input drift was not detected"\)/u,
    "one-hop append-opening negative control must only pass after detecting the exact Rust rejection drift",
  );

  const genericProofScalarBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-generic-proof-scalar-projection":'),
    guard.indexOf('if mode == "--negative-control-data-model-spend-proof-artifact-circuit-gates":'),
  );
  assertContainsAll(
    genericProofScalarBranch,
    [
      "self.public_inputs.recursive_proof_chain_digest",
      "self.public_inputs.transition_profile_binding_digest",
      "self.public_inputs.append_boundary_digest",
      "self.public_inputs.append_opening_preflight_digest",
      "self.public_inputs\\n                    .recursive_verifier_scalar_projection_digest",
      "recursive_proof_chain_digest",
      "transition_profile_binding_digest",
      "append_boundary_digest",
      "append_opening_preflight_digest",
      "recursive_verifier_scalar_projection_digest",
    ],
    "generic proof spend-state negative control must mutate every generic public-input spend-state field",
  );
  assert.match(
    genericProofScalarBranch,
    /for before, after, field in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "generic proof spend-state negative control must validate each mutated text snapshot",
  );
  assert.match(
    genericProofScalarBranch,
    /expected = f"missing Rust generic proof spend-state rejection for \{field\}"[\s\S]*?if expected not in message:/u,
    "generic proof spend-state negative control must require the exact field diagnostic",
  );
  assert.match(
    genericProofScalarBranch,
    /generic proof spend-state drift was not detected for[\s\S]*?print\("negative control rejected generic proof spend-state drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "generic proof spend-state negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    genericProofScalarBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "generic proof spend-state negative control must not unconditionally pass after run_checks",
  );

  const spendProofArtifactCircuitBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-spend-proof-artifact-circuit-gates":'),
    guard.indexOf('if mode == "--negative-control-data-model-previous-proof-opening-bundle-binding":'),
  );
  assertContainsAll(
    spendProofArtifactCircuitBranch,
    [
      "fn validate_kagemusha_recursive_spend_proof_public_input_binding(",
      "public_inputs.recursive_proof_chain_digest",
      "public_inputs.transition_profile_binding_digest",
      "if digest == [0u8; Hash::LENGTH]",
      "KagemushaRecursiveSpendProofCircuit::SemanticAggregation => {",
      "append_boundary_digest",
      "append_opening_preflight_digest",
      "recursive_verifier_scalar_projection_digest",
      "if digest != [0u8; Hash::LENGTH]",
      "KagemushaRecursiveSpendProofCircuit::Lineage => {",
      "if public_inputs.recursive_verifier_scalar_projection_digest == [0u8; Hash::LENGTH]",
      "if public_inputs.append_opening_preflight_digest != [0u8; Hash::LENGTH]",
      "&& public_inputs.append_boundary_digest == [0u8; Hash::LENGTH]",
    ],
    "spend proof artifact circuit-gate negative control must mutate top-level, semantic, and lineage gates",
  );
  assert.match(
    spendProofArtifactCircuitBranch,
    /for before, after, expected_marker in cases:[\s\S]*?case_index = source\.find\(before, function_start\)[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "spend proof artifact circuit-gate negative control must validate each scoped mutated text snapshot",
  );
  assert.match(
    spendProofArtifactCircuitBranch,
    /recursive spend proof artifact circuit gate binding is missing ordered preverification step:[\s\S]*?\+ expected_marker[\s\S]*?if expected not in message:/u,
    "spend proof artifact circuit-gate negative control must require the exact ordered-step diagnostic",
  );
  assert.match(
    spendProofArtifactCircuitBranch,
    /spend proof artifact circuit gate drift was not detected for[\s\S]*?print\("negative control rejected spend proof artifact circuit gate drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "spend proof artifact circuit-gate negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    spendProofArtifactCircuitBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "spend proof artifact circuit-gate negative control must not unconditionally pass after run_checks",
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
    /previous proof opening domain-tag bundle binding is missing ordered preverification step:[\s\S]*?previous_bundle\.validate_public_input_binding\(\)\?;[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: previous-proof opening bundle binding drift was not detected"\)/u,
    "previous-proof opening bundle-binding negative control must only pass after detecting the exact ordered-step drift",
  );

  const previousProofFieldBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-previous-proof-field-binding":'),
    guard.indexOf('if mode == "--negative-control-data-model-previous-proof-stale-hash-fixture":'),
  );
  assertContainsAll(
    previousProofFieldBranch,
    [
      "ensure_recursive_spend_previous_proof_matches",
      "domain",
      "evidence_digest",
      "folded_public_inputs_hash",
      "aggregation_transcript_digest",
      "verifier_params_fingerprint",
      "fixed_window_table_schedule_digest",
      "fixed_window_shared_table_manifest_digest",
      "fixed_window_table_base_digest",
      "verifier_witness_batch_digest",
      "recursive_proof_chain_digest",
      "transition_profile_binding_digest",
      "append_opening_preflight_digest",
      "append_boundary_digest",
      "recursive_verifier_scalar_projection_digest",
      "verifier_opening_len",
      "verifier_witness_count",
      "hop_count",
      "previous_recursive_proof.public_inputs.$field != expected.$field",
      'field: concat!("previous_recursive_proof.", stringify!($field))',
      "previous_recursive_proof.public_inputs_hash != expected.public_inputs_hash()?",
      'field: "previous_recursive_proof.public_inputs_hash"',
    ],
    "previous-proof field binding negative control must target every previous-proof public-input field and coverage marker",
  );
  assert.match(
    previousProofFieldBranch,
    /for field in fields:[\s\S]*?actual_fields = \[candidate for candidate in fields if candidate != field\][\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "previous-proof field binding negative control must validate each field-removal text snapshot",
  );
  assert.match(
    previousProofFieldBranch,
    /expected_prefix = "recursive spend previous-proof public-input field binding drifted;"[\s\S]*?expected_actual = f"actual=\{actual_fields\}"[\s\S]*?expected_prefix not in message or expected_actual not in message/u,
    "previous-proof field binding negative control must require the exact actual field-list diagnostic",
  );
  assert.match(
    previousProofFieldBranch,
    /for before, after in coverage_cases:[\s\S]*?case_index = source\.find\(before, function_start\)[\s\S]*?run_checks\(\)[\s\S]*?previous-proof field binding coverage drift was not detected for/u,
    "previous-proof field binding negative control must validate each macro/hash coverage mutation",
  );
  assert.match(
    previousProofFieldBranch,
    /recursive spend previous-proof public-input field binding is missing coverage:[\s\S]*?\+ before[\s\S]*?if expected not in message:/u,
    "previous-proof field binding negative control must require exact missing coverage diagnostics",
  );
  assert.match(
    previousProofFieldBranch,
    /previous-proof field binding drift was not detected for[\s\S]*?print\("negative control rejected previous-proof field binding drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "previous-proof field binding negative control must only pass after all injected drift is detected",
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
    guard.indexOf('if mode == "--negative-control-data-model-recursive-compact-constructor-binding":'),
  );
  assertContainsAll(
    previousProofStaleHashBranch,
    [
      "reserved_output_append_with_stale_previous_proof_payload",
      "reserved_output_append_with_unchecked_previous_proof_payload",
      "spliced previous proof folded public-input hash",
      "spliced previous proof unchecked folded public-input hash",
      'field: "previous_recursive_proof.folded_public_inputs_hash"',
      'field: "unchecked_previous_recursive_proof.folded_public_inputs_hash"',
      "recursive-spend-stale-previous-proof-public-input-hash",
      "recursive-spend-previous-proof-public-input-hash",
    ],
    "previous-proof stale hash negative control must mutate every stale/spliced previous-proof adversarial marker",
  );
  assert.match(
    previousProofStaleHashBranch,
    /for before, after, expected_marker, replace_all in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "previous-proof stale hash negative control must validate each mutated text snapshot",
  );
  assert.match(
    previousProofStaleHashBranch,
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?\+ expected_marker[\s\S]*?if expected not in message:/u,
    "previous-proof stale hash negative control must require exact adversarial fixture diagnostics",
  );
  assert.match(
    previousProofStaleHashBranch,
    /stale previous-proof hash fixture drift was not detected for[\s\S]*?print\("negative control rejected stale previous-proof hash fixture drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "previous-proof stale hash negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    previousProofStaleHashBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "previous-proof stale hash negative control must not unconditionally pass after run_checks",
  );

  const recursiveCompactConstructorBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-data-model-recursive-compact-constructor-binding":'),
    guard.indexOf('if mode == "--negative-control-core-offline-note-v2-retired-ios-app-attest-profile":'),
  );
  assertContainsAll(
    recursiveCompactConstructorBranch,
    [
      "zero recursive compact aggregation digest must reject token projection",
      'unsupported_proof_backend.proof = ProofBox::new("groth16/bn254".to_owned(), vec![0xA5]);',
      "unsupported_verifier_backend.verifier_key_id = VerifyingKeyId::new(",
      'backend_mismatch.proof = ProofBox::new("stark/fri".to_owned(), vec![0xA5]);',
      'non_halo2_backend.proof = ProofBox::new("stark/fri".to_owned(), vec![0xA5]);',
      "empty_proof.proof = ProofBox::new(",
      "halo2/ipa:kagemusha-recursive-unsupported",
      "recursive-compact-stale-proof-hash",
      "recursive-compact-spliced-transcript",
      "hop-count-spliced public-input hash",
    ],
    "recursive compact constructor negative control must mutate every constructor/adversarial marker",
  );
  assert.match(
    recursiveCompactConstructorBranch,
    /for before, after in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "recursive compact constructor negative control must validate each mutated text snapshot",
  );
  assert.match(
    recursiveCompactConstructorBranch,
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?\+ before[\s\S]*?if expected not in message:/u,
    "recursive compact constructor negative control must require exact adversarial marker diagnostics",
  );
  assert.match(
    recursiveCompactConstructorBranch,
    /recursive compact constructor binding drift was not detected for[\s\S]*?print\("negative control rejected recursive compact constructor binding drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "recursive compact constructor negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    recursiveCompactConstructorBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "recursive compact constructor negative control must not unconditionally pass after run_checks",
  );

  const offlineV2VectorPlatformBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-offline-v2-vector-platform-aliases":'),
    guard.indexOf('if mode == "--negative-control-offline-vector-platform-aliases":'),
  );
  assertContainsAll(
    workflow,
    ["ci/check_kagemusha_recursive_spend_policy.sh --negative-control-offline-v2-vector-platform-aliases"],
    "Kagemusha payload workflow must run the Offline V2 vector platform alias negative control",
  );
  assertContainsAll(
    guard,
    [
      "Offline V2 vector platform alias negative control",
      "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-offline-v2-vector-platform-aliases",
    ],
    "policy negative-control inventory must include the Offline V2 vector platform alias command",
  );
  assertContainsAll(
    offlineV2VectorPlatformBranch,
    [
      '"android-keymint" | "android" => (',
      '_ => (',
      'for platform in ["android", "ios-app-attest", "browser-webauthn"]',
      "for before, after, expected in mutations:",
    ],
    "Offline V2 vector platform alias negative control must mutate Android alias, wildcard fallback, and rejection-vector markers",
  );
  assert.match(
    offlineV2VectorPlatformBranch,
    /for before, after, expected in mutations:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "Offline V2 vector platform alias negative control must validate each mutated text snapshot",
  );
  assert.match(
    offlineV2VectorPlatformBranch,
    /if expected not in message:[\s\S]*?vector platform alias drift was rejected[\s\S]*?wrong reason/u,
    "Offline V2 vector platform alias negative control must require exact vector diagnostics",
  );
  assert.match(
    offlineV2VectorPlatformBranch,
    /vector platform alias drift was not detected for[\s\S]*?print\("negative control rejected Offline V2 vector platform alias drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "Offline V2 vector platform alias negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    offlineV2VectorPlatformBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Offline V2 vector platform alias negative control must not unconditionally pass after run_checks",
  );

  const offlineVectorPlatformBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-offline-vector-platform-aliases":'),
    guard.indexOf('if mode == "--negative-control-torii-offline-v2-kagemusha-redeem":'),
  );
  assertContainsAll(
    workflow,
    ["ci/check_kagemusha_recursive_spend_policy.sh --negative-control-offline-vector-platform-aliases"],
    "Kagemusha payload workflow must run the Offline vector platform alias negative control",
  );
  assertContainsAll(
    guard,
    [
      "Offline vector platform alias negative control",
      "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-offline-vector-platform-aliases",
    ],
    "policy negative-control inventory must include the Offline vector platform alias command",
  );
  assertContainsAll(
    offlineVectorPlatformBranch,
    [
      '"android-keymint" | "android" => (',
      '_ => (',
      'for platform in ["android", "ios-app-attest", "browser-webauthn"]',
      "for before, after, expected in mutations:",
    ],
    "Offline vector platform alias negative control must mutate Android alias, wildcard fallback, and rejection-vector markers",
  );
  assert.match(
    offlineVectorPlatformBranch,
    /for before, after, expected in mutations:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "Offline vector platform alias negative control must validate each mutated text snapshot",
  );
  assert.match(
    offlineVectorPlatformBranch,
    /if expected not in message:[\s\S]*?vector platform alias drift was rejected[\s\S]*?wrong reason/u,
    "Offline vector platform alias negative control must require exact vector diagnostics",
  );
  assert.match(
    offlineVectorPlatformBranch,
    /vector platform alias drift was not detected for[\s\S]*?print\("negative control rejected Offline vector platform alias drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "Offline vector platform alias negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    offlineVectorPlatformBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Offline vector platform alias negative control must not unconditionally pass after run_checks",
  );

  const toriiKagemushaRedeemBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-torii-offline-v2-kagemusha-redeem":'),
    guard.indexOf('if mode == "--negative-control-torii-offline-v2-kagemusha-openapi":'),
  );
  assertContainsAll(
    workflow,
    ["ci/check_kagemusha_recursive_spend_policy.sh --negative-control-torii-offline-v2-kagemusha-redeem"],
    "Kagemusha payload workflow must run the typed first-release Torii offline redeem ingress negative control",
  );
  assertContainsAll(
    guard,
    [
      "typed first-release Torii offline redeem ingress negative control",
      "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-torii-offline-v2-kagemusha-redeem",
    ],
    "policy negative-control inventory must include the typed first-release Torii offline redeem command",
  );
  assertContainsAll(
    toriiKagemushaRedeemBranch,
    [
      "crates/iroha_torii_shared/src/route_catalog.rs",
      'pub const REDEEM_PATH: &str = "/v1/offline/redeem";',
      "crates/iroha_torii_shared/src/offline_api.rs",
      "KagemushaRecursiveSpendRedeemRequest as OfflineRedeemRequest",
      "iroha_torii_shared::offline_api::OfflineRedeemRequest,",
      "&route_catalog::offline::REDEEM,",
      "validate_kagemusha_v2_redeem_snapshot(&app, &redeem_request)?;",
      "ensure_kagemusha_v2_backend_available()?;",
      "for target, before, after in cases:",
    ],
    "typed first-release Torii offline redeem negative control must mutate route, DTO, dispatch, snapshot, and backend gates",
  );
  assert.match(
    toriiKagemushaRedeemBranch,
    /for target, before, after in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "typed first-release Torii offline redeem negative control must validate each mutated ingress snapshot",
  );
  assert.match(
    toriiKagemushaRedeemBranch,
    /is missing typed first-release Torii offline redeem ingress coverage:[\s\S]*?\+ before[\s\S]*?if expected not in message:/u,
    "typed first-release Torii offline redeem negative control must require exact ingress diagnostics",
  );
  assert.match(
    toriiKagemushaRedeemBranch,
    /typed first-release Torii offline redeem ingress drift was not detected for[\s\S]*?print\("negative control rejected typed first-release Torii offline redeem ingress drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "typed first-release Torii offline redeem negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    toriiKagemushaRedeemBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "typed first-release Torii offline redeem negative control must not unconditionally pass after run_checks",
  );

  const toriiKagemushaOpenApiBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-torii-offline-v2-kagemusha-openapi":'),
    guard.indexOf('if mode == "--negative-control-torii-offline-v2-kagemusha-smoke":'),
  );
  assertContainsAll(
    workflow,
    ["ci/check_kagemusha_recursive_spend_policy.sh --negative-control-torii-offline-v2-kagemusha-openapi"],
    "Kagemusha payload workflow must run the typed first-release Torii offline redeem OpenAPI negative control",
  );
  assertContainsAll(
    guard,
    [
      "typed first-release Torii offline redeem OpenAPI negative control",
      "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-torii-offline-v2-kagemusha-openapi",
    ],
    "policy negative-control inventory must include the typed first-release Torii offline redeem OpenAPI command",
  );
  assertContainsAll(
    toriiKagemushaOpenApiBranch,
    [
      "crates/iroha_torii/src/openapi.rs",
      "docs/portal/static/openapi/torii.json",
      "docs/portal/static/openapi/versions/current/torii.json",
      'assert!(redeem_description.contains("directly encoded OfflineRedeemRequest"));',
      'assert!(redeem_description.contains("whole-payload base64 wrappers are rejected"));',
      '"/v1/offline/redeem": {',
      '"$ref": "#/components/schemas/OfflineRedeemRequest"',
      "for target, before, after in cases:",
    ],
    "typed first-release Torii offline redeem OpenAPI negative control must mutate source, latest, and current typed contracts",
  );
  assert.match(
    toriiKagemushaOpenApiBranch,
    /for target, before, after in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "typed first-release Torii offline redeem OpenAPI negative control must validate each mutated contract snapshot",
  );
  assert.match(
    toriiKagemushaOpenApiBranch,
    /is missing typed first-release Torii offline redeem ingress coverage:[\s\S]*?\+ before[\s\S]*?if expected not in message:/u,
    "typed first-release Torii offline redeem OpenAPI negative control must require exact ingress diagnostics",
  );
  assert.match(
    toriiKagemushaOpenApiBranch,
    /typed first-release Torii offline redeem OpenAPI drift was not detected for[\s\S]*?print\("negative control rejected typed first-release Torii offline redeem OpenAPI drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "typed first-release Torii offline redeem OpenAPI negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    toriiKagemushaOpenApiBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "typed first-release Torii offline redeem OpenAPI negative control must not unconditionally pass after run_checks",
  );

  const toriiKagemushaSmokeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-torii-offline-v2-kagemusha-smoke":'),
    guard.indexOf('if mode == "--negative-control-active-kagemusha-todo":'),
  );
  assertContainsAll(
    workflow,
    ["ci/check_kagemusha_recursive_spend_policy.sh --negative-control-torii-offline-v2-kagemusha-smoke"],
    "Kagemusha payload workflow must run the typed first-release Torii offline redeem smoke negative control",
  );
  assertContainsAll(
    guard,
    [
      "typed first-release Torii offline redeem smoke negative control",
      "ci/check_kagemusha_recursive_spend_policy.sh --negative-control-torii-offline-v2-kagemusha-smoke",
    ],
    "policy negative-control inventory must include the typed Torii offline redeem smoke command",
  );
  assertContainsAll(
    toriiKagemushaSmokeBranch,
    [
      'target = "crates/iroha_torii/tests/offline_redeem_contract.rs"',
      "redeem_is_a_typed_async_command_on_the_final_route",
      'TORII_SOURCE.contains("NoritoJson(request)")',
      "redeem_has_no_wrapper_or_compatibility_payload",
      "retired_redeem_routes_are_not_mounted",
      "for before, after in cases:",
    ],
    "typed Torii offline redeem smoke negative control must mutate every final-route assertion",
  );
  assert.match(
    toriiKagemushaSmokeBranch,
    /for before, after in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)[\s\S]*?text_overrides\.pop\(target, None\)/u,
    "typed Torii offline redeem smoke negative control must validate each mutated smoke snapshot",
  );
  assert.match(
    toriiKagemushaSmokeBranch,
    /is missing typed first-release Torii offline redeem ingress coverage:[\s\S]*?\+ before[\s\S]*?if expected not in message:/u,
    "typed Torii offline redeem smoke negative control must require exact ingress diagnostics",
  );
  assert.match(
    toriiKagemushaSmokeBranch,
    /typed first-release Torii offline redeem smoke drift was not detected for[\s\S]*?print\("negative control rejected typed first-release Torii offline redeem smoke drift"\)[\s\S]*?raise SystemExit\(0\)/u,
    "typed Torii offline redeem smoke negative control must only pass after all injected drift is detected",
  );
  assert.doesNotMatch(
    toriiKagemushaSmokeBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "typed first-release Torii offline redeem smoke negative control must not unconditionally pass after run_checks",
  );

  const coreAppendCapBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-append-cap-boundary":'),
    guard.indexOf('if mode == "--negative-control-core-lineage-profile-split":'),
  );
  assert.match(
    coreAppendCapBranch,
    /direct Reserved-lineage append at the witnessless hop cap must reject before input parsing[\s\S]*?direct Reserved-lineage append at the hop edge[\s\S]*?is missing Reserved-lineage adversarial coverage[\s\S]*?expected not in message/u,
    "core append cap boundary negative control must require the exact adversarial coverage diagnostic",
  );

  const profileSplitBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-lineage-profile-split":'),
    guard.indexOf('if mode == "--negative-control-core-proof-chain-accumulator":'),
  );
  assertContainsAll(
    profileSplitBranch,
    [
      "pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1: &str =",
      "pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1: &str =",
      "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1\\n            | KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
      "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 => {\\n            can_append_kagemusha_recursive_spend_lineage_witnessless(previous_hop_count)",
      "pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_CIRCUIT_ID: &str =",
      "pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_CIRCUIT_ID: &str =",
      "pub fn kagemusha_recursive_spend_lineage_append_vk_record(",
      'err.contains("is not `")\\n                && err.contains(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_CIRCUIT_ID)',
      'err.contains("is not `")\\n                && err.contains(KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_CIRCUIT_ID)',
      "one-hop lineage token must reject an append verifier record",
      "append lineage projected token must reject a one-hop verifier record",
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
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?proof-byte splice is bound into accumulator state[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: proof-chain accumulator drift was not detected"\)/u,
    "proof-chain accumulator negative control must only pass after detecting the exact adversarial coverage drift",
  );
  assert.doesNotMatch(
    proofChainBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "proof-chain accumulator negative control must not unconditionally pass after run_checks",
  );

  const tableBaseBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-fixed-window-table-base-accumulator":'),
    guard.indexOf('if mode == "--negative-control-core-shared-table-identity-base-selection":'),
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
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?per-hop fixed-window table-base digest must stream across append[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: fixed-window table-base accumulator drift was not detected"\)/u,
    "fixed-window table-base accumulator negative control must only pass after detecting the exact adversarial coverage drift",
  );
  assert.doesNotMatch(
    tableBaseBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "fixed-window table-base accumulator negative control must not unconditionally pass after run_checks",
  );

  const sharedTableIdentityBaseBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-shared-table-identity-base-selection":'),
    guard.indexOf('if mode == "--negative-control-core-shared-table-direct-mode-duplicate-witnesses":'),
  );
  assertContainsAll(
    guard,
    [
      "kagemusha_non_native_vesta_affine_windowed_shared_table_scalar_mul_accepts_identity_base",
      "kagemusha_non_native_vesta_affine_windowed_shared_table_scalar_mul_omits_direct_mode_duplicate_witnesses",
      "kagemusha_non_native_vesta_affine_windowed_shared_table_scalar_mul_synthesis_rejects_extra_direct_duplicate_witnesses",
      "kagemusha_non_native_vesta_affine_windowed_shared_table_scalar_mul_synthesis_rejects_missing_direct_window_base",
      "kagemusha_non_native_vesta_affine_windowed_shared_table_scalar_mul_rejects_scalar_bit_splice",
      "kagemusha_non_native_vesta_affine_windowed_shared_table_scalar_mul_rejects_direct_selected_addend_tamper",
      "kagemusha_non_native_vesta_affine_windowed_shared_table_native_scalar_msm_omits_direct_mode_duplicate_witnesses",
      "kagemusha_non_native_vesta_affine_windowed_shared_table_native_scalar_msm_accepts_identity_base_term",
      "kagemusha_non_native_vesta_ipa_verifier_shared_table_direct_one_bit_profile_uses_assigned_bases",
      "kagemusha_non_native_vesta_ipa_verifier_shared_table_direct_one_bit_public_instances_use_assigned_bases",
      "kagemusha_non_native_vesta_affine_native_scalar_mul_synthesis_rejects_extra_conditional_step",
      "kagemusha_non_native_vesta_affine_native_scalar_msm_synthesis_rejects_truncated_term_shape",
      "if !one_bit_direct_select {",
      "kagemusha_vesta_affine_windowed_shared_table_term_base(term),",
      ".and_then(|doubles| doubles.first())",
      "query_non_native_vesta_affine_windowed_shared_table_term_base::<",
      "ensure_witness_vector_len(config.conditional_adds.len(), self.conditional_adds.len())?;",
      "ensure_witness_vector_len(config.scalars.len(), witness.scalars.len())?;",
      "ensure_witness_vector_len(config.tables.len(), witness.tables.len())?;",
      "ensure_nested_witness_vector_lens(",
      "scalar_bit.clone() * current_base.2.clone()",
    ],
    "shared-table identity-base selection adversarial coverage",
  );
  assert.match(
    sharedTableIdentityBaseBranch,
    /scalar_bit\.clone\(\) \* current_base\.2\.clone\(\)[\s\S]*?unable to mutate shared-table identity-base selection/u,
    "shared-table identity-base selection negative control must remove the identity-base correction term",
  );
  assert.match(
    sharedTableIdentityBaseBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "shared-table identity-base selection negative control must validate the mutated text snapshot",
  );
  assert.match(
    sharedTableIdentityBaseBranch,
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?scalar_bit\.clone\(\) \* current_base\.2\.clone\(\)[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: shared-table identity-base selection drift was not detected"\)/u,
    "shared-table identity-base selection negative control must only pass after detecting the exact adversarial coverage drift",
  );
  assert.doesNotMatch(
    sharedTableIdentityBaseBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "shared-table identity-base selection negative control must not unconditionally pass after run_checks",
  );

  const sharedTableDirectModeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-shared-table-direct-mode-duplicate-witnesses":'),
    guard.indexOf('if mode == "--negative-control-core-shared-table-witness-shape-guards":'),
  );
  assert.match(
    sharedTableDirectModeBranch,
    /if !one_bit_direct_select \{[\s\S]*?if true \{/u,
    "shared-table direct-mode duplicate-witness negative control must remove the builder guard",
  );
  assert.match(
    sharedTableDirectModeBranch,
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?if !one_bit_direct_select \{[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: shared-table direct-mode duplicate-witness drift was not detected"\)/u,
    "shared-table direct-mode duplicate-witness negative control must only pass after detecting the exact adversarial coverage drift",
  );
  assert.doesNotMatch(
    sharedTableDirectModeBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "shared-table direct-mode duplicate-witness negative control must not unconditionally pass after run_checks",
  );

  const sharedTableWitnessShapeBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-shared-table-witness-shape-guards":'),
    guard.indexOf('if mode == "--negative-control-core-shared-table-direct-base-helper":'),
  );
  assert.match(
    sharedTableWitnessShapeBranch,
    /ensure_witness_vector_len\(config\.tables\.len\(\), witness\.tables\.len\(\)\)\?;[\s\S]*?let _ = \(config\.tables\.len\(\), witness\.tables\.len\(\)\);/u,
    "shared-table witness-shape negative control must remove the table-vector length guard",
  );
  assert.match(
    sharedTableWitnessShapeBranch,
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?ensure_witness_vector_len\(config\.tables\.len\(\), witness\.tables\.len\(\)\)\?;[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: shared-table witness-shape guard drift was not detected"\)/u,
    "shared-table witness-shape negative control must only pass after detecting the exact guard drift",
  );
  assert.doesNotMatch(
    sharedTableWitnessShapeBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "shared-table witness-shape negative control must not unconditionally pass after run_checks",
  );

  const sharedTableDirectBaseBranch = guard.slice(
    guard.indexOf('if mode == "--negative-control-core-shared-table-direct-base-helper":'),
    guard.indexOf('if mode == "--negative-control-core-append-boundary-accumulator":'),
  );
  assertContainsAll(
    sharedTableDirectBaseBranch,
    [
      "kagemusha_vesta_affine_windowed_shared_table_term_base(term),",
      ".and_then(|doubles| doubles.first())",
      "shared-table direct-base helper drift was rejected for the wrong reason",
    ],
    "shared-table direct-base helper negative control must mutate assigned-base helper coverage",
  );
  assert.match(
    sharedTableDirectBaseBranch,
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?kagemusha_vesta_affine_windowed_shared_table_term_base\(term\),[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: shared-table direct-base helper drift was not detected"\)/u,
    "shared-table direct-base helper negative control must only pass after detecting exact helper drift",
  );
  assert.doesNotMatch(
    sharedTableDirectBaseBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "shared-table direct-base helper negative control must not unconditionally pass after run_checks",
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
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?append-boundary digest must not feed back into the accumulator digest[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append-boundary accumulator drift was not detected"\)/u,
    "append-boundary accumulator negative control must only pass after detecting the exact adversarial coverage drift",
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
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?field: "append_boundary\.previous_accumulator_digest"[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: previous accumulator boundary drift was not detected"\)/u,
    "previous accumulator boundary negative control must only pass after detecting the exact adversarial coverage drift",
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
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?refresh_append_boundary_digest\(&mut self_consistent_forged_public_inputs\);[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: append-boundary public inputs refresh drift was not detected"\s*\)/u,
    "append-boundary public-inputs negative control must only pass after detecting the exact refresh diagnostic",
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
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?field: "append_boundary\.verifier_params_fingerprint"[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: append-boundary verifier context refresh drift was not detected"\s*\)/u,
    "append-boundary verifier-context negative control must only pass after detecting the exact verifier-context diagnostic",
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
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?refresh_append_boundary_digest\(&mut self_consistent_forged_hop_count\);[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\(\s*"negative control failed: append-boundary hop-count refresh drift was not detected"\s*\)/u,
    "append-boundary hop-count negative control must only pass after detecting the exact refresh diagnostic",
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
    /is missing Reserved-lineage adversarial coverage:[\s\S]*?append_boundary\.append_boundary_digest != accumulator\.append_boundary_digest[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append-boundary digest match drift was not detected"\)/u,
    "append-boundary digest match negative control must only pass after detecting the exact digest comparator diagnostic",
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
    /unchecked append digest helper must remain private:[\s\S]*?kagemusha_recursive_spend_lineage_append_opening_preflight_digest_unchecked[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append digest unchecked surface drift was not detected"\)/u,
    "append digest unchecked surface negative control must only pass after detecting the exact private-helper diagnostic",
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
    /append opening preflight public digest wrapper is missing ordered preverification step:[\s\S]*?preflight\.validate_context\(\)\?;[\s\S]*?expected not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append digest wrapper bypass drift was not detected"\)/u,
    "append digest wrapper bypass negative control must only pass after detecting the exact checked-wrapper diagnostic",
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
    /append boundary transition-profile comparison fields drifted;[\s\S]*?expected_actual = \(\n\s*"'fixed_window_table_schedule_digest', 'fixed_window_shared_table_manifest_digest'\]"\n\s*\)[\s\S]*?expected_prefix not in message or expected_actual not in message[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: append-boundary profile comparison drift was not detected"\)/u,
    "append-boundary profile comparison negative control must only pass after detecting the exact field-list drift",
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
    /expected_prefix = "recursive aggregation public-input schema order drifted;"[\s\S]*?expected_actual = \([\s\S]*?append_boundary_digest_limb0[\s\S]*?append_opening_preflight_digest_limb0[\s\S]*?expected_prefix not in message or expected_actual not in message[\s\S]*?wrong reason/u,
    "recursive public-input schema negative control must require the exact schema-order diagnostic",
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
    /expected = \([\s\S]*?recursive aggregation public-input index map drifted:[\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_BOUNDARY_START_INDEX expected 48 actual 44[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
    "recursive public-input index negative control must require the exact index-map diagnostic",
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
    /expected_prefix = "recursive aggregation public-input value builder order drifted;"[\s\S]*?expected_actual = \([\s\S]*?append_boundary_limbs\[0\][\s\S]*?append_opening_preflight_limbs\[0\][\s\S]*?expected_prefix not in message or expected_actual not in message[\s\S]*?wrong reason/u,
    "recursive public-input value negative control must require the exact value-order diagnostic",
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
    /expected_prefix = "recursive aggregation non-zero public field groups drifted;"[\s\S]*?expected_actual = \([\s\S]*?\[28, 29, 30, 31\], \[28, 29, 30, 31\]\][\s\S]*?expected_prefix not in message or expected_actual not in message[\s\S]*?wrong reason/u,
    "recursive public-input nonzero group negative control must require the exact group diagnostic",
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
    /expected_prefix = "append recursive verifier-slice semantic non-zero groups drifted;"[\s\S]*?expected_actual = \([\s\S]*?KAGEMUSHA_RECURSIVE_AGGREGATION_APPEND_OPENING_PREFLIGHT_START_INDEX[\s\S]*?append-boundary digest[\s\S]*?expected_prefix not in message or expected_actual not in message[\s\S]*?wrong reason/u,
    "append semantic nonzero group negative control must require the exact semantic group diagnostic",
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
    "--negative-control-core-fold-public-input-binding",
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
  assert.match(
    guard,
    /fn kagemusha_verified_folded_public_inputs_rejects_public_input_bindings_before_proof_verify[\s\S]*?wrong chain id[\s\S]*?Kagemusha fold confidential-v2 chain tag mismatch[\s\S]*?wrong asset definition[\s\S]*?Kagemusha fold confidential-v2 asset tag mismatch[\s\S]*?metadata root_before splice[\s\S]*?Kagemusha fold confidential-v2 root mismatch[\s\S]*?metadata nullifier splice[\s\S]*?Kagemusha fold confidential-v2 nullifier mismatch[\s\S]*?metadata output commitment splice[\s\S]*?Kagemusha fold confidential-v2 output commitment mismatch/u,
    "policy guard must pin checked-fold public-input binding mismatch coverage",
  );

  const bindingStart = guard.indexOf('if mode == "--negative-control-core-fold-public-input-binding":');
  const bindingEnd = guard.indexOf('if mode == "--negative-control-core-fold-public-input-preverify-order":');
  const bindingBranch = guard.slice(bindingStart, bindingEnd);
  assert.match(
    bindingBranch,
    /direct public-input binding mismatch must reject before proof verification[\s\S]*?direct public-input binding may verify proof first/u,
    "checked-fold public-input binding negative control must weaken binding coverage",
  );
  assert.match(
    bindingBranch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "checked-fold public-input binding negative control must validate the mutated text snapshot",
  );
  assert.match(
    bindingBranch,
    /expected = \([\s\S]*?direct public-input binding mismatch must reject before proof verification[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
    "checked-fold public-input binding negative control must require the exact missing-coverage diagnostic",
  );
  assert.match(
    bindingBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: checked-fold public-input binding drift was not detected"\)/u,
    "checked-fold public-input binding negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    bindingBranch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "checked-fold public-input binding negative control must not unconditionally pass after run_checks",
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
    /expected = \([\s\S]*?checked-fold direct public-input preverification path is missing ordered preverification step:[\s\S]*?verified_steps\.push\(kagemusha_verified_fold_step\(step\)\?\);[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
    "checked-fold public-input negative control must require the exact ordered-step diagnostic",
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
    /expected = \([\s\S]*?record-backed checked-fold public-input preverification path is missing ordered preverification step:[\s\S]*?validate_required_kagemusha_confidential_v2_step_public_inputs\([\s\S]*?expected not in message[\s\S]*?wrong reason/u,
    "record-backed checked-fold public-input negative control must require the exact ordered-step diagnostic",
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
      "lineage witness root-continuity error should come before Pallas archive decoding",
    ],
    [
      "--negative-control-core-lineage-witness-record-predecode",
      /lineage witness verifier-record error should come before Pallas archive decoding[\s\S]*?lineage witness verifier-record error may decode Pallas first/u,
      "lineage witness verifier-record predecode",
      "lineage witness verifier-record error should come before Pallas archive decoding",
    ],
    [
      "--negative-control-core-lineage-witness-count-mismatch-predecode",
      /current-note count mismatch: expected 2, found 1[\s\S]*?current-note count mismatch: expected 2, found 0/u,
      "lineage witness count-mismatch predecode",
      "current-note count mismatch: expected 2, found 1",
    ],
    [
      "--negative-control-core-lineage-witness-envelope-count",
      /lineage envelope count mismatch: expected 2, found 0[\s\S]*?lineage envelope count mismatch: expected 2, found 1[\s\S]*?if envelopes\.len\(\) != step_count[\s\S]*?if false && envelopes\.len\(\) != step_count/u,
      "lineage witness envelope-count",
      null,
    ],
    [
      "--negative-control-core-lineage-witness-malformed-envelope-archive",
      /kagemusha_recursive_spend_lineage_witness_rejects_malformed_envelope_archive[\s\S]*?kagemusha_recursive_spend_lineage_witness_allows_malformed_envelope_archive/u,
      "lineage witness malformed envelope archive",
      "fn kagemusha_recursive_spend_lineage_witness_rejects_malformed_envelope_archive",
    ],
    [
      "--negative-control-core-lineage-witness-note-predecode",
      /lineage witness current-note error should come before Pallas archive decoding[\s\S]*?lineage witness current-note error may decode Pallas first/u,
      "lineage witness current-note predecode",
      "lineage witness current-note error should come before Pallas archive decoding",
    ],
    [
      "--negative-control-core-lineage-witness-note-binding-predecode",
      /lineage witness current-note binding error should come before Pallas archive decoding[\s\S]*?lineage witness current-note binding error may decode Pallas first/u,
      "lineage witness current-note binding predecode",
      "lineage witness current-note binding error should come before Pallas archive decoding",
    ],
    [
      "--negative-control-core-lineage-witness-current-note-invariants",
      /current note 0 spend nullifier must be non-zero[\s\S]*?current note 0 spend nullifier may be zero/u,
      "lineage witness current-note invariants",
      "current note 0 spend nullifier must be non-zero",
    ],
    [
      "--negative-control-core-lineage-witness-handoff-predecode",
      /lineage witness append-handoff error should come before Pallas archive decoding[\s\S]*?lineage witness append-handoff error may decode Pallas first/u,
      "lineage witness append-handoff predecode",
      "lineage witness append-handoff error should come before Pallas archive decoding",
    ],
    [
      "--negative-control-core-lineage-witness-duplicate-current-note",
      /current note 2 spend nullifier is duplicated[\s\S]*?current note 2 spend nullifier may be duplicated/u,
      "lineage witness duplicate current-note spend-nullifier",
      "current note 2 spend nullifier is duplicated",
    ],
    [
      "--negative-control-core-lineage-witness-final-bundle-context",
      /fixed_bytes\(b\\"kagemusha-lineage-context-spliced-final-nullifier\\"\)[\s\S]*?fixed_bytes\(b\\"kagemusha-lineage-context-spliced-final-nullifier-unchecked\\"\)/u,
      "lineage witness final-bundle context",
      'fixed_bytes(b\\"kagemusha-lineage-context-spliced-final-nullifier\\")',
    ],
    [
      "--negative-control-core-lineage-witness-final-bundle-predecode",
      /lineage witness final-bundle error should come before Pallas archive decoding[\s\S]*?lineage witness final-bundle error may decode Pallas first/u,
      "lineage witness final-bundle predecode",
      "lineage witness final-bundle error should come before Pallas archive decoding",
    ],
  ];

  for (const [mode, mutationPattern, label, diagnosticNeedle] of branchSpecs) {
    const branch = policyBranch(mode);
    assert.match(branch, mutationPattern, `${label} negative control must mutate the guarded source text`);
    assert.match(
      branch,
      /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
      `${label} negative control must validate the mutated text snapshot`,
    );
    if (mode === "--negative-control-core-lineage-witness-envelope-count") {
      assert.match(
        branch,
        /cases\s*=\s*\([\s\S]*?for target, before, after, label in cases:[\s\S]*?except\s+PolicyError\s+as\s+error:[\s\S]*?continue[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed:[\s\S]*?raise\s+SystemExit\(0\)/u,
        `${label} negative control must only pass after detecting every injected drift`,
      );
    } else {
      assert.match(
        branch,
        /expected = \([\s\S]*?\{target\} is missing Reserved-lineage adversarial coverage:[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
        `${label} negative control must require the exact missing-coverage diagnostic`,
      );
      assert.ok(
        branch.includes(diagnosticNeedle),
        `${label} negative control must pin its exact missing coverage needle`,
      );
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

test("recursive Kagemusha policy negative controls pin ABI-7 compact adversarial coverage", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-core-recursive-compact-public-instance-shape",
    "--negative-control-core-recursive-compact-pallas-count",
    "--negative-control-core-recursive-compact-pallas-metadata",
    "--negative-control-core-recursive-compact-cid-spoof-key",
    "--negative-control-core-recursive-spend-compact-projection-token",
    "--negative-control-core-recursive-compact-unanchored-prover-surface",
    "--negative-control-core-reserved-lineage-circuit-id-wording",
    "--negative-control-core-recursive-compact-key-package-wording",
    "--negative-control-core-folded-path-wording",
    "--negative-control-core-offline-first-release-test-wording",
    "--negative-control-core-retired-backend-profile-wording",
    "--negative-control-core-offline-note-v2-retired-ios-app-attest-profile",
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
      "recursive compact token multi-row public instances must reject",
    ],
    [
      "--negative-control-core-recursive-compact-pallas-count",
      /if envelopes\.len\(\) != step_count[\s\S]*?if false && envelopes\.len\(\) != step_count[\s\S]*?detached compact Pallas archive must reject before proving[\s\S]*?detached compact Pallas archive may return unavailable[\s\S]*?height-aware detached compact Pallas archive must reject before proving[\s\S]*?height-aware detached compact Pallas archive may return unavailable[\s\S]*?extra compact Pallas opening must reject before proving[\s\S]*?extra compact Pallas opening may return unavailable[\s\S]*?height-aware extra compact Pallas opening must reject before proving[\s\S]*?height-aware extra compact Pallas opening may return unavailable[\s\S]*?missing compact Pallas opening must reject before proving[\s\S]*?missing compact Pallas opening may return unavailable[\s\S]*?height-aware missing compact Pallas opening must reject before proving[\s\S]*?height-aware missing compact Pallas opening may return unavailable[\s\S]*?duplicated multi-hop compact Pallas archive must reject before proving[\s\S]*?duplicated multi-hop compact Pallas archive may return unavailable[\s\S]*?height-aware duplicated multi-hop compact Pallas archive must reject before proving[\s\S]*?height-aware duplicated multi-hop compact Pallas archive may return unavailable[\s\S]*?reordered multi-hop compact Pallas archive must reject before proving[\s\S]*?reordered multi-hop compact Pallas archive may return unavailable[\s\S]*?height-aware reordered multi-hop compact Pallas archive must reject before proving[\s\S]*?height-aware reordered multi-hop compact Pallas archive may return unavailable/u,
      "core recursive compact Pallas opening count",
      null,
    ],
    [
      "--negative-control-core-recursive-compact-pallas-metadata",
      /forged multi-hop compact Pallas metadata must reject before proving[\s\S]*?forged multi-hop compact Pallas metadata may return unavailable[\s\S]*?height-aware forged multi-hop compact Pallas metadata must reject before proving[\s\S]*?height-aware forged multi-hop compact Pallas metadata may return unavailable/u,
      "core recursive compact Pallas metadata",
      null,
    ],
    [
      "--negative-control-core-recursive-compact-cid-spoof-key",
      /\.expect_err\("CID-spoofed ABI-7 compact verifier key must reject"\);[\s\S]*?\.expect_err\("CID-spoofed ABI-7 compact verifier key may pass"\);[\s\S]*?\.expect_err\("public CID-spoofed ABI-7 compact verifier key must reject"\);[\s\S]*?\.expect_err\("public CID-spoofed ABI-7 compact verifier key may pass"\);/u,
      "core recursive compact CID-spoof key",
      null,
    ],
    [
      "--negative-control-core-recursive-spend-compact-projection-token",
      /pub fn verify_kagemusha_recursive_spend_compact_payment_token_projection\([\s\S]*?pub fn verify_kagemusha_recursive_spend_compact_payment_token_projection_unchecked\(/u,
      "core recursive spend compact projection token",
      "pub fn verify_kagemusha_recursive_spend_compact_payment_token_projection(",
    ],
    [
      "--negative-control-core-recursive-compact-unanchored-prover-surface",
      /pub fn prove_verified_kagemusha_compact_payment_token\(/u,
      "core recursive compact first-release public surface",
      "pub fn prove_verified_kagemusha_compact_payment_token(",
    ],
    [
      "--negative-control-core-reserved-lineage-circuit-id-wording",
      /Canonical Reserved-lineage circuit-family identifier[\s\S]*?Legacy Reserved-lineage circuit-family identifier/u,
      "core Reserved-lineage circuit id first-release wording",
      "/// Canonical Reserved-lineage circuit-family identifier for recursive spend lineage proofs.",
    ],
    [
      "--negative-control-core-recursive-compact-key-package-wording",
      /one_hop_proving_key_bytes: Option<&\[u8\]>[\s\S]*?one_hop_fallback_proving_key_bytes: Option<&\[u8\]>/u,
      "core recursive compact key-package wording",
      "one_hop_proving_key_bytes: Option<&[u8]>",
    ],
    [
      "--negative-control-core-folded-path-wording",
      /rejects_recursive_mode_on_folded_path[\s\S]*?rejects_recursive_mode_on_retired_path/u,
      "core Kagemusha folded-path first-release wording",
      "fn prove_kagemusha_compact_payment_token_rejects_recursive_mode_on_folded_path()",
    ],
    [
      "--negative-control-core-offline-first-release-test-wording",
      /retired v1 tree root[\s\S]*?leg[\s\S]*?acy tree root[\s\S]*?non-zk1 transcript payload[\s\S]*?leg[\s\S]*?acy transcript payload[\s\S]*?issuer signature should authorize without attestation marker[\s\S]*?valid fallback[\s\S]*?crates\/iroha_config\/src\/parameters\/user\.rs[\s\S]*?removed Kagemusha force flag must not parse[\s\S]*?leg[\s\S]*?acy readiness knob must not parse[\s\S]*?removed Kagemusha force flag should produce a parse error/u,
      "core Offline/Kagemusha first-release test wording",
      "partial recursive redeem change output must not record the retired v1 tree root",
    ],
    [
      "--negative-control-core-retired-backend-profile-wording",
      /is_retired_unqualified_vote_bool_backend_profile[\s\S]*?is_legacy_vote_bool_backend_profile[\s\S]*?is_retired_unqualified_anon_transfer_backend_profile[\s\S]*?is_legacy_anon_transfer_backend_profile[\s\S]*?retired unqualified profile roots[\s\S]*?legacy profile roots/u,
      "core retired backend profile wording",
      "is_retired_unqualified_vote_bool_backend_profile",
    ],
    [
      "--negative-control-core-offline-note-v2-retired-ios-app-attest-profile",
      /OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST_LEGACY[\s\S]*?retired ios app attest spelling[\s\S]*?accepted retired ios app attest spelling[\s\S]*?offline_app_attest_signature_compat_total/u,
      "core Offline Note V2 retired iOS App Attest profile",
      "contains retired Offline Note V2 iOS App Attest certificate profile",
    ],
    [
      "--negative-control-bridge-recursive-compact-public-instance-shape",
      /ABI-7 compact verifier must reject multi-row public instances before returning a soft invalid result[\s\S]*?ABI-7 compact verifier may soft-invalid multi-row public instances/u,
      "bridge recursive compact public-instance shape",
      "ABI-7 compact verifier must reject multi-row public instances before returning a soft invalid result",
    ],
    [
      "--negative-control-bridge-recursive-compact-pallas-count",
      /ABI-7 compact prover must reject extra valid Pallas opening archives before proving[\s\S]*?ABI-7 compact prover may accept extra valid Pallas opening archives[\s\S]*?ABI-7 compact prover must reject missing valid Pallas opening archives before proving[\s\S]*?ABI-7 compact prover may accept missing valid Pallas opening archives[\s\S]*?ABI-7 compact prover must reject duplicated multi-hop valid Pallas opening archives before proving[\s\S]*?ABI-7 compact prover may accept duplicated multi-hop valid Pallas opening archives[\s\S]*?ABI-7 compact prover must reject reordered valid Pallas opening archives before proving[\s\S]*?ABI-7 compact prover may accept reordered valid Pallas opening archives/u,
      "bridge recursive compact Pallas opening count",
      null,
    ],
    [
      "--negative-control-bridge-recursive-compact-pallas-metadata",
      /ABI-7 compact prover must reject forged multi-hop Pallas metadata before proving[\s\S]*?ABI-7 compact prover may accept forged multi-hop Pallas metadata/u,
      "bridge recursive compact Pallas metadata",
      "ABI-7 compact prover must reject forged multi-hop Pallas metadata before proving",
    ],
    [
      "--negative-control-bridge-recursive-compact-vk-hash",
      /ABI-7 compact verifier must reject non-canonical envelope verifier-key hashes before returning a soft invalid result[\s\S]*?ABI-7 compact verifier may soft-invalid non-canonical envelope verifier-key hashes/u,
      "bridge recursive compact verifier-key hash",
      "ABI-7 compact verifier must reject non-canonical envelope verifier-key hashes before returning a soft invalid result",
    ],
    [
      "--negative-control-js-host-recursive-compact-vk-hash",
      /recursive compact token with forged verifier-key hash must reject[\s\S]*?recursive compact token with forged verifier-key hash may soft-invalid/u,
      "JS host recursive compact verifier-key hash",
      "recursive compact token with forged verifier-key hash must reject",
    ],
    [
      "--negative-control-js-host-recursive-compact-pallas-count",
      /recursive compact prover must reject extra valid Pallas opening archive[\s\S]*?recursive compact prover may accept extra valid Pallas opening archive[\s\S]*?recursive compact prover must reject missing valid Pallas opening archive[\s\S]*?recursive compact prover may accept missing valid Pallas opening archive[\s\S]*?recursive compact prover must reject duplicated multi-hop valid Pallas opening archive[\s\S]*?recursive compact prover may accept duplicated multi-hop valid Pallas opening archive[\s\S]*?recursive compact prover must reject reordered valid Pallas opening archive[\s\S]*?recursive compact prover may accept reordered valid Pallas opening archive/u,
      "JS host recursive compact Pallas opening count",
      null,
    ],
    [
      "--negative-control-js-host-recursive-compact-pallas-metadata",
      /recursive compact prover must reject forged multi-hop Pallas metadata[\s\S]*?recursive compact prover may accept forged multi-hop Pallas metadata/u,
      "JS host recursive compact Pallas metadata",
      "recursive compact prover must reject forged multi-hop Pallas metadata",
    ],
    [
      "--negative-control-js-host-recursive-compact-public-instance-shape",
      /JS host recursive compact verifier must reject multi-row public instances[\s\S]*?JS host recursive compact verifier may soft-invalid multi-row public instances/u,
      "JS host recursive compact public-instance shape",
      "JS host recursive compact verifier must reject multi-row public instances",
    ],
    [
      "--negative-control-python-recursive-compact-vk-hash",
      /recursive compact token with forged verifier-key hash must reject[\s\S]*?recursive compact token with forged verifier-key hash may soft-invalid/u,
      "Python recursive compact verifier-key hash",
      "recursive compact token with forged verifier-key hash must reject",
    ],
    [
      "--negative-control-python-recursive-compact-pallas-count",
      /recursive compact prover must reject extra valid Pallas opening archive[\s\S]*?recursive compact prover may accept extra valid Pallas opening archive[\s\S]*?recursive compact prover must reject missing valid Pallas opening archive[\s\S]*?recursive compact prover may accept missing valid Pallas opening archive[\s\S]*?recursive compact prover must reject duplicated multi-hop valid Pallas opening archive[\s\S]*?recursive compact prover may accept duplicated multi-hop valid Pallas opening archive[\s\S]*?recursive compact prover must reject reordered valid Pallas opening archive[\s\S]*?recursive compact prover may accept reordered valid Pallas opening archive/u,
      "Python recursive compact Pallas opening count",
      null,
    ],
    [
      "--negative-control-python-recursive-compact-pallas-metadata",
      /recursive compact prover must reject forged multi-hop Pallas metadata[\s\S]*?recursive compact prover may accept forged multi-hop Pallas metadata/u,
      "Python recursive compact Pallas metadata",
      "recursive compact prover must reject forged multi-hop Pallas metadata",
    ],
    [
      "--negative-control-python-recursive-compact-public-instance-shape",
      /Python recursive compact verifier must reject multi-row public instances[\s\S]*?Python recursive compact verifier may soft-invalid multi-row public instances/u,
      "Python recursive compact public-instance shape",
      "Python recursive compact verifier must reject multi-row public instances",
    ],
  ];

  for (const [mode, mutationPattern, label, diagnosticNeedle] of branchSpecs) {
    const branch = policyBranch(mode);
    assert.match(branch, mutationPattern, `${label} negative control must mutate the guarded source text`);
    assert.match(
      branch,
      /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
      `${label} negative control must validate the mutated text snapshot`,
    );
    if (mode === "--negative-control-core-recursive-compact-unanchored-prover-surface") {
      assert.match(
        branch,
        /expected = \([\s\S]*?exposes retired unanchored compact-token prover surface/u,
        `${label} negative control must require the exact retired-surface diagnostic`,
      );
      assert.match(
        branch,
        /recursive compact unanchored prover surface drift was not detected/u,
        `${label} negative control must fail when injected drift is not detected`,
      );
    } else if (mode === "--negative-control-core-retired-backend-profile-wording") {
      assert.match(
        branch,
        /replacements\s*=\s*\([\s\S]*?for before, after in replacements:[\s\S]*?contains stale recursive compact first-release wording[\s\S]*?is missing Reserved-lineage adversarial coverage/u,
        `${label} negative control must check both stale and missing first-release wording diagnostics`,
      );
      assert.match(
        branch,
        /if first_message is None:[\s\S]*?retired backend profile wording drift was not detected[\s\S]*?raise\s+SystemExit\(0\)/u,
        `${label} negative control must only pass after every retired-profile mutation is detected`,
      );
    } else if (mode === "--negative-control-core-offline-note-v2-retired-ios-app-attest-profile") {
      assert.match(
        branch,
        /mutations\s*=\s*\([\s\S]*?contains retired Offline Note V2 iOS App Attest certificate profile[\s\S]*?is missing retired Offline Note V2 iOS App Attest profile rejection coverage[\s\S]*?contains retired Offline Note V2 iOS App Attest compatibility telemetry[\s\S]*?for target, before, after, expected in mutations:/u,
        `${label} negative control must check source, rejection-test, and telemetry diagnostics`,
      );
      assert.match(
        branch,
        /if not detected_messages:[\s\S]*?Offline Note V2 retired iOS App Attest profile drift was not detected[\s\S]*?raise\s+SystemExit\(0\)/u,
        `${label} negative control must only pass after every retired-profile mutation is detected`,
      );
    } else if (mode === "--negative-control-core-offline-first-release-test-wording") {
      assert.match(
        branch,
        /cases\s*=\s*\([\s\S]*?crates\/iroha_config\/src\/parameters\/user\.rs[\s\S]*?removed Kagemusha force flag must not parse[\s\S]*?iroha_config Kagemusha force flag tests contain stale first-release wording[\s\S]*?for target, before, after, expected in cases:[\s\S]*?if expected not in message:[\s\S]*?wrong reason/u,
        `${label} negative control must check each stale first-release wording diagnostic`,
      );
      assert.ok(
        branch.includes(diagnosticNeedle),
        `${label} negative control must pin its exact stale wording needle`,
      );
      assert.match(
        branch,
        /if first_message is None:[\s\S]*?test wording drift was not detected[\s\S]*?raise\s+SystemExit\(0\)/u,
        `${label} negative control must only pass after every wording mutation is detected`,
      );
    } else if (
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
        /expected = \([\s\S]*?\{target\} is missing Reserved-lineage adversarial coverage:[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
        `${label} negative control must require the exact missing-coverage diagnostic`,
      );
      assert.ok(
        branch.includes(diagnosticNeedle),
        `${label} negative control must pin its exact missing coverage needle`,
      );
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
    /expected = \([\s\S]*?\{target\} is missing Reserved-lineage adversarial coverage:[\s\S]*?shared_table_batch_preflight_rejects_h_generator_fold_splice[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
    "core Vesta IPA H-fold negative control must require the exact missing-coverage diagnostic",
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
    /expected = \([\s\S]*?\{target\} is missing Reserved-lineage adversarial coverage:[\s\S]*?from_pallas_witness_rejects_generator_fold_splice[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
    "core Vesta IPA G-fold negative control must require the exact missing-coverage diagnostic",
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
    /expected = \([\s\S]*?\{target\} is missing Reserved-lineage adversarial coverage:[\s\S]*?pub struct KagemushaRecursiveSpendLineageAppendOpeningPreflight \{[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
    "append-opening preflight negative control must require the exact missing-coverage diagnostic",
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

test("recursive Kagemusha policy negative controls pin core verifier-slice predecode diagnostics", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const expectedModes = [
    "--negative-control-core-current-hop-opening-metadata-splice",
    "--negative-control-core-one-hop-verifier-slice-evidence-binding",
    "--negative-control-core-fold-overlap-predecode",
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

  const branchSpecs = [
    [
      "--negative-control-core-current-hop-opening-metadata-splice",
      "--negative-control-core-append-verifier-slice-preflight-binding",
      /metadata-spliced current-hop opening archive must reject[\s\S]*?metadata-spliced current-hop opening archive may pass/u,
      /expected = \([\s\S]*?metadata-spliced current-hop opening archive must reject[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
      "current-hop opening metadata splice",
    ],
    [
      "--negative-control-core-one-hop-verifier-slice-evidence-binding",
      "--negative-control-core-fold-overlap-predecode",
      /one-hop verifier-slice evidence binding must reject params fingerprint splice[\s\S]*?one-hop verifier-slice evidence binding may accept params fingerprint splice/u,
      /expected = \([\s\S]*?one-hop verifier-slice evidence binding must reject params fingerprint splice[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
      "one-hop verifier-slice evidence binding",
    ],
    [
      "--negative-control-core-fold-overlap-predecode",
      "--negative-control-core-fold-public-input-binding",
      /record-backed cross-hop overlap error should come before proof decoding[\s\S]*?record-backed cross-hop overlap may decode proof first/u,
      /expected = \([\s\S]*?record-backed cross-hop overlap error should come before proof decoding[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
      "checked-fold overlap predecode",
    ],
  ];

  for (const [mode, nextMode, mutationPattern, diagnosticPattern, label] of branchSpecs) {
    const branch = guard.slice(
      guard.indexOf(`if mode == "${mode}":`),
      guard.indexOf(`if mode == "${nextMode}":`),
    );
    assert.match(branch, mutationPattern, `${label} negative control must mutate the guarded source text`);
    assert.match(
      branch,
      /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
      `${label} negative control must validate the mutated text snapshot`,
    );
    assert.match(
      branch,
      diagnosticPattern,
      `${label} negative control must require the exact missing-coverage diagnostic`,
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
      "'err.contains(\"backend `stark/fri` is not\"),'",
    ],
    "core adversarial coverage must pin each previous-proof verifier-context splice fragment",
  );
  assert.doesNotMatch(
    adversarialCoverage,
    /^\s+"backend `stark\/fri` is not",$/mu,
    "core adversarial coverage must not pin the shadowable bare previous-proof backend diagnostic",
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
    /err\.contains\("backend `stark\/fri` is not"\),[\s\S]*?err\.contains\("backend `stark\/fri` may pass"\),[\s\S]*?previous proof verifier-key backend mismatch must reject[\s\S]*?previous proof verifier-key backend mismatch may pass[\s\S]*?unsupported previous proof circuit id must reject[\s\S]*?unsupported previous proof circuit id may pass/u,
    "core previous-proof backend profile negative control must mutate proof-backend, verifier-key, and circuit-id coverage",
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

test("recursive Kagemusha policy adversarial coverage avoids non-C# duplicate and substring-shadow needles", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const blockStart = guard.indexOf("ADVERSARIAL_COVERAGE = {");
  const blockEnd = guard.indexOf("SDK_HELPER_EDGE_COVERAGE = {", blockStart);
  assert.notEqual(blockStart, -1, "policy guard must define ADVERSARIAL_COVERAGE");
  assert.notEqual(blockEnd, -1, "policy guard must terminate ADVERSARIAL_COVERAGE before SDK helper coverage");
  const block = guard.slice(blockStart, blockEnd);
  const entries = [
    ...block.matchAll(/^    "(?<path>[^"]+)": \(\n(?<body>[\s\S]*?)^    \),/gmu),
  ];
  assert.ok(entries.length > 0, "policy guard must expose adversarial coverage entries");

  const literalNeedles = (body) =>
    body
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.endsWith(","))
      .map((line) => line.slice(0, -1))
      .filter((line) => line.length >= 2 && ["\"", "'"].includes(line[0]) && line.at(-1) === line[0])
      .map((line) => line.slice(1, -1));

  for (const match of entries) {
    const path = match.groups.path;
    if (path.startsWith("csharp/")) {
      continue;
    }
    const needles = literalNeedles(match.groups.body);
    const counts = new Map();
    for (const needle of needles) {
      counts.set(needle, (counts.get(needle) ?? 0) + 1);
    }
    const duplicates = [...counts]
      .filter(([, count]) => count > 1)
      .map(([needle, count]) => `${count}x ${needle}`);
    assert.deepEqual(duplicates, [], `${path} adversarial coverage needles must be duplicate-free`);

    const shadows = [];
    for (const needle of needles) {
      if (needle.length < 12) {
        continue;
      }
      const shadow = needles.find((other) => other !== needle && other.includes(needle));
      if (shadow !== undefined) {
        shadows.push(`${needle} -> ${shadow}`);
      }
    }
    assert.deepEqual(shadows, [], `${path} adversarial coverage needles must avoid substring shadows`);
  }
});

test("recursive Kagemusha profile-split coverage avoids non-C# duplicate and substring-shadow needles", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const blockStart = guard.indexOf("RESERVED_LINEAGE_PROFILE_SPLIT_COVERAGE = {");
  const blockEnd = guard.indexOf("VERIFY_RESULT_FAIL_CLOSED_COVERAGE = {", blockStart);
  assert.notEqual(blockStart, -1, "policy guard must define RESERVED_LINEAGE_PROFILE_SPLIT_COVERAGE");
  assert.notEqual(blockEnd, -1, "policy guard must terminate profile-split coverage before verify-result coverage");
  const block = guard.slice(blockStart, blockEnd);
  const entries = [
    ...block.matchAll(/^    "(?<path>[^"]+)": \(\n(?<body>[\s\S]*?)^    \),/gmu),
  ];
  assert.ok(entries.length > 0, "policy guard must expose profile-split coverage entries");

  const literalNeedles = (body) =>
    body
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.endsWith(","))
      .map((line) => line.slice(0, -1))
      .filter((line) => line.length >= 2 && ["\"", "'"].includes(line[0]) && line.at(-1) === line[0])
      .map((line) => line.slice(1, -1));

  for (const match of entries) {
    const path = match.groups.path;
    if (path.startsWith("csharp/")) {
      continue;
    }
    const needles = literalNeedles(match.groups.body);
    const counts = new Map();
    for (const needle of needles) {
      counts.set(needle, (counts.get(needle) ?? 0) + 1);
    }
    const duplicates = [...counts]
      .filter(([, count]) => count > 1)
      .map(([needle, count]) => `${count}x ${needle}`);
    assert.deepEqual(duplicates, [], `${path} profile-split coverage needles must be duplicate-free`);

    const shadows = [];
    for (const needle of needles) {
      if (needle.length < 12) {
        continue;
      }
      const shadow = needles.find((other) => other !== needle && other.includes(needle));
      if (shadow !== undefined) {
        shadows.push(`${needle} -> ${shadow}`);
      }
    }
    assert.deepEqual(shadows, [], `${path} profile-split coverage needles must avoid substring shadows`);
  }
});

test("recursive Kagemusha verify-result fail-closed coverage avoids non-C# duplicate and substring-shadow needles", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-verify-result-flags";

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

  const blockStart = guard.indexOf("VERIFY_RESULT_FAIL_CLOSED_COVERAGE = {");
  const blockEnd = guard.indexOf("PAYLOAD_BENCH_SOURCE_COVERAGE = {", blockStart);
  assert.notEqual(blockStart, -1, "policy guard must define VERIFY_RESULT_FAIL_CLOSED_COVERAGE");
  assert.notEqual(blockEnd, -1, "policy guard must terminate verify-result coverage before payload-bench coverage");
  const block = guard.slice(blockStart, blockEnd);
  assertContainsAll(
    block,
    [
      "iroha_data_model::offline::can_redeem_kagemusha_recursive_spend_witnessless(\\n            &bundle.recursive_proof.verifier_key_id.name,\\n            bundle.accumulator.hop_count,",
      "let lineage_witness_required_for_redeem = !witnessless_redeem_supported;",
      "iroha_data_model::offline::can_redeem_kagemusha_recursive_spend_witnessless(\\n                &two_hop_lineage_bundle.recursive_proof.verifier_key_id.name,\\n                two_hop_lineage_bundle.accumulator.hop_count,",
      "metadata-valid two-hop append lineage profile must remain fail-closed until circuit-authenticated recursion is wired",
    ],
    "verify-result fail-closed coverage",
  );
  const entries = [
    ...block.matchAll(/^    "(?<path>[^"]+)": \(\n(?<body>[\s\S]*?)^    \),/gmu),
  ];
  assert.ok(entries.length > 0, "policy guard must expose verify-result coverage entries");
  const literalNeedles = (body) =>
    body
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.endsWith(","))
      .map((line) => line.slice(0, -1))
      .filter((line) => line.length >= 2 && ["\"", "'"].includes(line[0]) && line.at(-1) === line[0])
      .map((line) => line.slice(1, -1));

  for (const match of entries) {
    const path = match.groups.path;
    if (path.startsWith("csharp/")) {
      continue;
    }
    const needles = literalNeedles(match.groups.body);
    const counts = new Map();
    for (const needle of needles) {
      counts.set(needle, (counts.get(needle) ?? 0) + 1);
    }
    const duplicates = [...counts]
      .filter(([, count]) => count > 1)
      .map(([needle, count]) => `${count}x ${needle}`);
    assert.deepEqual(duplicates, [], `${path} verify-result coverage needles must be duplicate-free`);

    const shadows = [];
    for (const needle of needles) {
      if (needle.length < 12) {
        continue;
      }
      const shadow = needles.find((other) => other !== needle && other.includes(needle));
      if (shadow !== undefined) {
        shadows.push(`${needle} -> ${shadow}`);
      }
    }
    assert.deepEqual(shadows, [], `${path} verify-result coverage needles must avoid substring shadows`);
  }

  const branch = guard.slice(
    guard.indexOf(`if mode == "${mode}":`),
    guard.indexOf("if mode:", guard.indexOf(`if mode == "${mode}":`)),
  );
  assert.match(
    branch,
    /cases\s*=\s*\([\s\S]*?bundle\.accumulator\.hop_count[\s\S]*?lineage_witness_required_for_redeem = !witnessless_redeem_supported[\s\S]*?two_hop_lineage_bundle\.accumulator\.hop_count[\s\S]*?for before, after, label in cases:/u,
    "verify-result fail-closed negative control must mutate runtime flags and two-hop witnessless coverage",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?verify-result flag drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise SystemExit\("negative control failed: verify-result flag drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "verify-result fail-closed negative control must only pass after every case detects injected drift",
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

test("recursive Kagemusha policy negative controls pin JS append-boundary output-set exactness", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-js-host-append-boundary-current-output-set";

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
    guard.indexOf('if mode == "--negative-control-js-host-append-boundary-current-output-set":'),
    guard.indexOf('if mode == "--negative-control-python-recursive-compact-vk-hash":'),
  );
  assert.match(
    branch,
    /JS host append-boundary helper must reject duplicate current-hop outputs[\s\S]*?JS host append-boundary helper may accept duplicate current-hop outputs[\s\S]*?repeats an output commitment[\s\S]*?accepts duplicate output commitment/u,
    "JS append-boundary output-set negative control must weaken duplicate-output coverage",
  );
  assert.match(
    branch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "JS append-boundary output-set negative control must validate the mutated text snapshot",
  );
  assert.match(
    branch,
    /expected = \([\s\S]*?\{target\} is missing Reserved-lineage adversarial coverage:[\s\S]*?JS host append-boundary helper must reject duplicate current-hop outputs[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
    "JS append-boundary output-set negative control must require the exact missing-coverage diagnostic",
  );
  assert.match(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: JS host append-boundary current-hop output-set drift was not detected"\)/u,
    "JS append-boundary output-set negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS append-boundary output-set negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin JS append-boundary forged-result exactness", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-js-host-append-boundary-forged-result-hashes";

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
    guard.indexOf('if mode == "--negative-control-js-host-append-boundary-forged-result-hashes":'),
    guard.indexOf('if mode == "--negative-control-python-recursive-compact-vk-hash":'),
  );
  assertContainsAll(
    branch,
    [
      "fn kagemusha_recursive_spend_lineage_append_boundary_rejects_forged_result_hashes",
      "fn kagemusha_recursive_spend_lineage_append_boundary_accepts_forged_result_hashes",
      "JS host append-boundary helper must reject forged resulting accumulator digest",
      "JS host append-boundary helper may accept forged resulting accumulator digest",
      "JS host append-boundary helper must reject forged resulting public-input hash",
      "JS host append-boundary helper may accept forged resulting public-input hash",
      "accepted_resulting_accumulator_digest",
      "accepted_resulting_public_inputs_hash",
    ],
    "JS append-boundary forged-result negative control must pin test, assertion, and exact field coverage",
  );
  assert.match(
    branch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "JS append-boundary forged-result negative control must validate each mutated text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?JS host append-boundary forged result drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: JS host append-boundary forged result drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "JS append-boundary forged-result negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "JS append-boundary forged-result negative control must not unconditionally pass after run_checks",
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

test("recursive Kagemusha policy negative controls pin Python append-boundary forged-result exactness", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-python-append-boundary-forged-result-hashes";

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
    guard.indexOf('if mode == "--negative-control-python-append-boundary-forged-result-hashes":'),
    guard.indexOf('if mode == "--negative-control-fixed-window-manifest-digest-splice":'),
  );
  assertContainsAll(
    branch,
    [
      "fn kagemusha_recursive_spend_lineage_append_boundary_python_rejects_forged_result_hashes",
      "fn kagemusha_recursive_spend_lineage_append_boundary_python_accepts_forged_result_hashes",
      "Python append-boundary helper must reject forged resulting accumulator digest",
      "Python append-boundary helper may accept forged resulting accumulator digest",
      "Python append-boundary helper must reject forged resulting public-input hash",
      "Python append-boundary helper may accept forged resulting public-input hash",
      "accepted_resulting_accumulator_digest",
      "accepted_resulting_public_inputs_hash",
    ],
    "Python append-boundary forged-result negative control must pin test, assertion, and exact field coverage",
  );
  assert.match(
    branch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "Python append-boundary forged-result negative control must validate each mutated text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?Python append-boundary forged result drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: Python append-boundary forged result drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "Python append-boundary forged-result negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "Python append-boundary forged-result negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha policy negative controls pin fixed-window manifest digest splice", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-fixed-window-manifest-digest-splice";

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
    guard.indexOf('if mode == "--negative-control-fixed-window-manifest-digest-splice":'),
    guard.indexOf('if mode == "--negative-control-workflow":'),
  );
  assert.match(
    branch,
    /manifest row splice must change digest[\s\S]*?manifest row splice should change digest/u,
    "fixed-window manifest negative control must weaken the manifest splice assertion",
  );
  assert.match(
    branch,
    /text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "fixed-window manifest negative control must validate the mutated text snapshot",
  );
  assert.match(
    branch,
    /expected = \([\s\S]*?\{target\} is missing Reserved-lineage one-hop\/append profile split coverage:[\s\S]*?manifest row splice must change digest[\s\S]*?expected not in message[\s\S]*?wrong reason/u,
    "fixed-window manifest negative control must require the exact profile-split diagnostic",
  );
  assert.match(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)[\s\S]*?raise\s+SystemExit\("negative control failed: fixed-window manifest digest splice drift was not detected"\)/u,
    "fixed-window manifest negative control must only pass after detecting injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "fixed-window manifest negative control must not unconditionally pass after run_checks",
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

test("recursive Kagemusha policy negative controls pin bridge append-boundary forged-result exactness", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_policy.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const mode = "--negative-control-bridge-append-boundary-forged-result-hashes";

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
    guard.indexOf('if mode == "--negative-control-bridge-append-boundary-forged-result-hashes":'),
    guard.indexOf('if mode == "--negative-control-js-host-recursive-compact-vk-hash":'),
  );
  assertContainsAll(
    branch,
    [
      "forged_resulting_accumulator_profile",
      "accepted_resulting_accumulator_profile",
      "forged resulting accumulator digest",
      "accepted resulting accumulator digest",
      "forged_resulting_public_inputs_profile",
      "accepted_resulting_public_inputs_profile",
      "forged resulting public-input hash",
      "accepted resulting public-input hash",
    ],
    "bridge append-boundary forged-result negative control must pin both forged result hash cases",
  );
  assert.match(
    branch,
    /for before, after, label in cases:[\s\S]*?text_overrides\[target\]\s*=\s*mutated[\s\S]*?run_checks\(\)/u,
    "bridge append-boundary forged-result negative control must validate each mutated text snapshot",
  );
  assert.match(
    branch,
    /if label not in message:[\s\S]*?bridge append-boundary forged result drift was not detected for[\s\S]*?if first_message is None:[\s\S]*?raise\s+SystemExit\("negative control failed: bridge append-boundary forged result drift was not detected"\)[\s\S]*?raise\s+SystemExit\(0\)/u,
    "bridge append-boundary forged-result negative control must only pass after every case detects injected drift",
  );
  assert.doesNotMatch(
    branch,
    /except\s+PolicyError\s+as\s+error:[\s\S]*?raise\s+SystemExit\(0\)\s*raise\s+SystemExit\(0\)/u,
    "bridge append-boundary forged-result negative control must not unconditionally pass after run_checks",
  );
});

test("recursive Kagemusha Swift/native V2 parity guard pins exact live inventories", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_sdk_parity.sh");
  const stringInventory = (start, end, label) => {
    const startIndex = guard.indexOf(start);
    const endIndex = guard.indexOf(end, startIndex + start.length);
    assert.notEqual(startIndex, -1, `${label} start marker must exist`);
    assert.notEqual(endIndex, -1, `${label} end marker must exist`);
    const values = namesFromMatches(
      guard.slice(startIndex, endIndex),
      /^\s*"([^"]+)",?$/gmu,
    );
    assert.equal(new Set(values).size, values.length, `${label} must not contain duplicates`);
    return new Set(values);
  };

  const requiredFileSection = guard.slice(
    guard.indexOf("REQUIRED_FILES = ("),
    guard.indexOf("ALLOWED_SWIFT_OFFLINE_SOURCE_FILES ="),
  );
  assertSameSet(
    new Set(namesFromMatches(requiredFileSection, /^\s+([A-Z][A-Z_]+),$/gmu)),
    [
      "SWIFT_PROTOCOL",
      "SWIFT_CODECS",
      "SWIFT_NATIVE",
      "SWIFT_AMOUNT",
      "SWIFT_TORII_MODELS",
      "SWIFT_TORII_CLIENT",
      "SWIFT_TX_BUILDER",
      "SWIFT_ATTESTATION",
      "NATIVE_RUST",
      "NATIVE_HEADER",
      "NATIVE_UMBRELLA_HEADER",
    ],
    "Swift/native V2 parity required files",
  );

  const allowedSwiftSources = stringInventory(
    "ALLOWED_SWIFT_OFFLINE_SOURCE_FILES = frozenset(",
    "ALLOWED_SWIFT_OFFLINE_TEST_FILES = frozenset(",
    "Swift offline source inventory",
  );
  assertSameSet(
    allowedSwiftSources,
    [
      "KagemushaRecursiveSpendV2.swift",
      "KagemushaRecursiveSpendV2Codecs.swift",
      "KagemushaRecursiveSpendV2Native.swift",
      "KagemushaScaledAmount.swift",
      "OfflineDeviceAttestation.swift",
      "ToriiKagemushaAPIModels.swift",
    ],
    "retained Swift offline source inventory",
  );
  const allowedSwiftTests = stringInventory(
    "ALLOWED_SWIFT_OFFLINE_TEST_FILES = frozenset(",
    "ALLOWED_SWIFT_KAGEMUSHA_PUBLIC_TYPES = frozenset(",
    "Swift offline test inventory",
  );
  assertSameSet(
    allowedSwiftTests,
    [
      "KagemushaRecursiveSpendV2Tests.swift",
      "KagemushaScaledAmountTests.swift",
      "ToriiKagemushaAPIModelsTests.swift",
    ],
    "retained Swift offline test inventory",
  );

  const swiftSourceRoot = new URL(
    "IrohaSwift/Sources/IrohaSwift/",
    `file://${REPO_ROOT}/`,
  );
  const actualOfflineSources = new Set(
    readdirSync(swiftSourceRoot).filter(
      (name) => name.endsWith(".swift") && (name.includes("Kagemusha") || name.includes("Offline")),
    ),
  );
  assertSameSet(
    actualOfflineSources,
    allowedSwiftSources,
    "guarded Swift offline source inventory",
  );
  const swiftTestRoot = new URL(
    "IrohaSwift/Tests/IrohaSwiftTests/",
    `file://${REPO_ROOT}/`,
  );
  const actualOfflineTests = new Set(
    readdirSync(swiftTestRoot).filter(
      (name) => name.endsWith(".swift") && (name.includes("Kagemusha") || name.includes("Offline")),
    ),
  );
  assertSameSet(actualOfflineTests, allowedSwiftTests, "guarded Swift offline test inventory");

  const publicTypeInventory = stringInventory(
    "ALLOWED_SWIFT_KAGEMUSHA_PUBLIC_TYPES = frozenset(",
    "REQUIRED_NATIVE_EXPORTS = (",
    "Swift Kagemusha public type inventory",
  );
  const actualPublicTypes = new Set();
  for (const name of readdirSync(swiftSourceRoot).filter((entry) => entry.endsWith(".swift"))) {
    for (const typeName of namesFromMatches(
      source(`IrohaSwift/Sources/IrohaSwift/${name}`),
      /^public\s+(?:final\s+)?(?:struct|enum|class|protocol|typealias)\s+(Kagemusha\w+)/gmu,
    )) {
      actualPublicTypes.add(typeName);
    }
  }
  assertSameSet(
    actualPublicTypes,
    publicTypeInventory,
    "guarded Swift Kagemusha public type inventory",
  );

  const nativeExportInventory = stringInventory(
    "REQUIRED_NATIVE_EXPORTS = (",
    "class CheckFailure",
    "Kagemusha native export inventory",
  );
  assertSameSet(
    nativeExportInventory,
    REQUIRED_KAGEMUSHA_V2_NATIVE_SYMBOLS,
    "guarded ABI-18 Kagemusha native exports",
  );
  for (const [relative, label] of [
    ["crates/connect_norito_bridge/src/lib.rs", "Rust bridge"],
    ["crates/connect_norito_bridge/include/connect_norito_bridge.h", "C header"],
    ["IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2Native.swift", "Swift V2 loader"],
  ]) {
    assertSameSet(
      new Set(namesFromMatches(source(relative), /\b(connect_norito_kagemusha_[a-z0-9_]+)\b/gu)),
      nativeExportInventory,
      `${label} exact native export inventory`,
    );
  }

  assertContainsAll(
    guard,
    [
      "actual_source_files != ALLOWED_SWIFT_OFFLINE_SOURCE_FILES",
      "actual_test_files != ALLOWED_SWIFT_OFFLINE_TEST_FILES",
      "swift_public_types != ALLOWED_SWIFT_KAGEMUSHA_PUBLIC_TYPES",
      "actual_native_exports != expected_native_exports",
      "if len(cases) != 1:",
      'case recursiveSpend = "recursive_spend_v1"',
      'public static let productMode = "recursive_spend_v1"',
      'public static let artifactManifestMode = "recursive_spend_v2"',
      "public static let mode = productMode",
      "value == productMode",
      "public struct KagemushaScaledAmount",
      "public struct KagemushaNoteOpening",
      "public struct KagemushaRecipientOutputDerivationRequest",
      "public struct KagemushaRecursiveSpendInitRequest",
      "public struct KagemushaRecursiveSpendAppendRequest",
      "public struct KagemushaRecursiveSpendVerifyRequest",
      "public struct KagemushaRecursiveSpendVerifierRecordRef",
      "public struct KagemushaRecursiveSpendRedeemUnsigned",
      "public struct KagemushaRecursiveSpendRedeemRequest",
      "public struct KagemushaRecursiveSpendRedeemChangeBuildRequest",
      "public struct KagemushaRecursiveSpendLineageWitness",
      "public static func initSpend(",
      "public static func appendSpend(",
      "public static func verifySpend(",
      "public static func redeemSpend(",
      "public enum KagemushaDeviceAttestation",
      "public enum KagemushaDeviceAttestationError",
      "public func prepareKagemushaTopUpShield(",
      "expectedReadiness: KagemushaTopUpShieldReadinessExpectation",
      'case readiness = "/v1/offline/readiness"',
      'case topUp = "/v1/offline/top-up"',
      'case redeem = "/v1/offline/redeem"',
      'case operations = "/v1/offline/operations"',
      "public struct OfflineTopUpRequest",
      "public struct OfflineRedeemRequest",
      "public enum OfflineOperationStatus",
      '"Content-Type": "application/x-norito"',
      '"Accept": "application/x-norito"',
      '"Idempotency-Key": operationId',
      "try ensureStatus(response, equals: 202",
      "actual_routes != expected_routes",
      're.search(r"base64", swift_models, re.I)',
      "Swift direct Torii request models must carry canonical Norito bytes only",
      `require(native_umbrella, '#include "connect_norito_bridge.h"', "native umbrella header")`,
    ],
    "Swift/native V2 parity invariants",
  );
});

test("recursive Kagemusha Swift/native V2 parity negative controls stay workflow-backed", () => {
  const guard = source("ci/check_kagemusha_recursive_spend_sdk_parity.sh");
  const workflow = source(".github/workflows/pr_kagemusha_payload_bench.yml");
  const negativeModes = [
    "--negative-control-extra-swift-protocol",
    "--negative-control-extra-native-export",
    "--negative-control-product-mode",
    "--negative-control-direct-route",
    "--negative-control-required-native-export",
  ];
  const negativeControlSection = guard.slice(
    guard.indexOf("negative_controls = {"),
    guard.indexOf("if mode:"),
  );
  assertSameSet(
    new Set(
      namesFromMatches(
        negativeControlSection,
        /^\s{4}"(--negative-control-[^"]+)":/gmu,
      ),
    ),
    negativeModes,
    "Swift/native V2 parity negative-control inventory",
  );
  assertContainsAll(
    negativeControlSection,
    [
      "SWIFT_PROTOCOL",
      "KagemushaAlternativeSpendProtocol",
      "Swift Kagemusha public symbol inventory",
      "NATIVE_HEADER",
      "connect_norito_kagemusha_alternative_spend",
      "Kagemusha native export inventory",
      "Swift product mode",
      "SWIFT_TORII_MODELS",
      "Swift direct Torii route",
    ],
    "Swift/native V2 parity negative-control targets and expected failures",
  );
  assertContainsAll(
    guard,
    [
      "if mode not in negative_controls:",
      "unsupported parity mode:",
      'public static let productMode = "recursive_spend_v9"',
      'case topUp = "/v1/offline/v2/kagemusha/topup"',
      "connect_norito_kagemusha_recursive_spend_append_removed_v2",
      "negative control failed for the wrong reason:",
      "negative control rejected drift:",
      "negative control was not rejected:",
      "Kagemusha SDK parity failed:",
      "Kagemusha Swift/native single-protocol parity passed",
    ],
    "Swift/native V2 parity negative-control execution contract",
  );
  assertWorkflowRunsNegativeControlModes(
    workflow,
    "ci/check_kagemusha_recursive_spend_sdk_parity.sh",
    negativeModes,
    "Swift/native V2 parity guard",
  );
  assert.match(
    workflow,
    /^\s+run: ci\/check_kagemusha_recursive_spend_sdk_parity\.sh$/mu,
    "Swift/native V2 parity workflow must run the positive guard",
  );
});
