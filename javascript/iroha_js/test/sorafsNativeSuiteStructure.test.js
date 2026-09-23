// Static and synthetic registration only; no SDK/native assertion body executes.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import { inspectEntrypoint, inspectRegistration } from "./helpers/sorafsNativeSuiteContract.js";
import { createNativeTestHelper } from "./helpers/nativeRequirements.js";

const TEST_ROOT = path.dirname(fileURLToPath(import.meta.url));
const REPOSITORY_ROOT = path.resolve(TEST_ROOT, "../../..");
const contract = JSON.parse(readFileSync(path.join(TEST_ROOT, "fixtures/sorafs_native_suite_contract_v1.json"), "utf8"));
const subjectNames = Object.freeze({
  cancelAssetLockV1: ["decodeCancelAssetLockV1", "encodeCancelAssetLockV1"],
  sorafsAppealFinanceValidation: ["validateAppealFinanceCancelAssetLock"],
  sorafsFixtureBundleValidation: ["SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1", "SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS", "validateFixtureBundle"],
  sorafsOrderbookSubmission: ["LocalSigningContext", "SorafsOrderbookSubmissionAmbiguousError", "ToriiClient", "NetworkId", "TORII_TEST_NATIVE_BINDING"],
  "sorafsOrchestrator.parity": ["getNativeBinding"],
  sorafsPdpValidation: ["SORAFS_PDP_PAYLOAD_KINDS", "validatePdpBundle", "validatePdpChallengeProof", "validatePdpCommitmentChallenge", "validatePdpPayload"],
});
function readSuite(row) {
  return readFileSync(path.join(TEST_ROOT, "sorafsNativeSuites", row.name + ".js"), "utf8");
}
function checkSuite(row, source) {
  const { name, registration, statement_count, original_sha256, entrypoint_sha256, ...expected } = row;
  assert.deepEqual(inspectRegistration(source, registration, statement_count), expected, name);
}
function directoryUrl(...parts) {
  return pathToFileURL(path.join(...parts) + path.sep);
}

test("shared SoraFS suites retain all 172 assertions and the complete 46 plus nine case inventory", () => {
  assert.equal(contract.schema, "sorafs.javascript.shared_assertion_contract.v1");
  assert.equal(contract.suites.length, 6);
  assert.equal(contract.suites.reduce((sum, row) => sum + row.assertion_count, 0), 172);
  assert.equal(contract.suites.reduce((sum, row) => sum + row.cases.length, 0), 46);
  assert.equal(contract.suites.reduce((sum, row) => sum + row.nested_case_names.length, 0), 9);
  for (const row of contract.suites) {
    checkSuite(row, readSuite(row));
    assert.equal(inspectEntrypoint(readFileSync(path.join(TEST_ROOT, row.name + ".test.js"), "utf8")), row.entrypoint_sha256);
  }
});

for (const row of contract.suites) {
  test(row.name + ": contract rejects changed assertion, case, source import and context prefix", () => {
    const source = readSuite(row);
    const firstCase = JSON.stringify(row.cases[0].name);
    const mutations = [
      source.replace("assert.", "assert.changed_"),
      source.replace(firstCase, JSON.stringify(row.cases[0].name + " changed")),
      'import "../../src/native.js";\n' + source,
      source.replace("(context) {", "(context) {\n  const substituted = null;"),
      source.replace(/\btest\(/u, "test.skip("),
    ];
    for (const changed of mutations) {
      assert.notEqual(changed, source, "mutation must change the tested source");
      assert.throws(() => checkSuite(row, changed));
    }
  });
  test(row.name + ": synthetic registration never calls SDK subjects", async () => {
    const captured = [];
    const register = (name, callback) => { captured.push({ name, callback }); };
    const forbidden = () => { throw new Error("synthetic registration must not execute SDK/native methods"); };
    const subject = Object.fromEntries(subjectNames[row.name].map((name) => [name, forbidden]));
    if (row.name === "sorafsFixtureBundleValidation") {
      subject.SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1 = 128;
      // Only registration builds profile closures; their native callbacks do not run.
      subject.SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS = Object.freeze({});
    }
    if (row.name === "sorafsOrderbookSubmission") {
      subject.NetworkId = Object.freeze({ parse(value) {
        assert.equal(value, "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
        return Object.freeze({ syntheticRegistrationIdentity: true });
      } });
      subject.TORII_TEST_NATIVE_BINDING = Symbol("synthetic-registration-only");
    }
    const context = { test: register, subject };
    if (row.name !== "sorafsOrderbookSubmission") {
      const relative = row.name === "sorafsOrchestrator.parity"
        ? "fixtures/sorafs_orchestrator/multi_peer_parity_v1"
        : ["cancelAssetLockV1", "sorafsAppealFinanceValidation"].includes(row.name)
          ? "fixtures/sorafs_manifest/appeal_finance" : "fixtures/sorafs_manifest";
      context.fixtureRoot = directoryUrl(REPOSITORY_ROOT, relative);
    }
    if (["sorafsFixtureBundleValidation", "sorafsOrchestrator.parity"].includes(row.name)) {
      context.nativeBinding = { sorafsValidateFixtureBundleJson: forbidden };
      context.nativeBindingError = null;
    }
    if (row.name === "sorafsOrchestrator.parity") {
      context.repositoryRoot = directoryUrl(REPOSITORY_ROOT);
      context.temporaryRoot = path.join(REPOSITORY_ROOT, "target");
    }
    const module = await import(pathToFileURL(path.join(TEST_ROOT, "sorafsNativeSuites", row.name + ".js")).href);
    module[row.registration](context);
    assert.deepEqual(captured.map((item) => item.name), row.cases.map((item) => item.name));
    if (row.nested_case_names.length) {
      const outer = captured.find((item) => item.name === "fixture-bundle wrapper matches all nine release-wide outcome goldens byte-for-byte");
      const nested = [];
      await outer.callback({ test(name, callback) { nested.push({ name, callback }); } });
      assert.deepEqual(nested.map((item) => item.name), row.nested_case_names);
      assert.ok(nested.every((item) => typeof item.callback === "function"));
    }
  });
}

test("pure native requirements preserve the original factory AST without source loading", () => {
  const pure = readFileSync(path.join(TEST_ROOT, "helpers/nativeRequirements.js"), "utf8");
  const eager = readFileSync(path.join(TEST_ROOT, "helpers/native.js"), "utf8");
  assert.equal(inspectEntrypoint(pure), contract.native_requirement_ast_sha256);
  assert.equal(inspectEntrypoint(eager), contract.native_eager_ast_sha256);
  assert.doesNotMatch(pure, /\bimport\b|\brequire\s*\(/u);
  assert.notEqual(inspectEntrypoint(pure.replace("throw createError();", "return undefined;")), contract.native_requirement_ast_sha256);
});

test("pure native requirements fail missing loads and preserve errors without skipping", () => {
  const originalError = new Error("original selected native load failed");
  const helper = createNativeTestHelper(null, originalError);
  const cases = [];
  const wrapped = helper.makeNativeTest((name, optionsOrFn, maybeFn) => {
    cases.push({ name, options: typeof optionsOrFn === "function" ? undefined : optionsOrFn,
      callback: typeof optionsOrFn === "function" ? optionsOrFn : maybeFn });
  });
  wrapped("native missing", { timeout: 1000 }, () => assert.fail("missing native body must not execute"));
  assert.equal(cases.length, 1);
  assert.equal(cases[0].name, "native missing");
  assert.deepEqual(cases[0].options, { timeout: 1000 });
  assert.throws(cases[0].callback, (error) => {
    assert.equal(error.code, "ERR_IROHA_NATIVE_TEST_REQUIREMENT");
    assert.equal(error.cause, originalError);
    return true;
  });
});

test("pure native requirements reject missing symbols and preserve successful runner identity", () => {
  const selected = { present() {} };
  const helper = createNativeTestHelper(selected, null);
  const cases = [];
  const runner = (name, callback) => cases.push({ name, callback });
  assert.equal(helper.makeNativeTest(runner, { require: "present" }), runner);
  helper.makeNativeTest(runner, { require: ["present", "absent"] })("missing callable", () => assert.fail("must fail"));
  assert.throws(cases[0].callback, (error) => {
    assert.equal(error.code, "ERR_IROHA_NATIVE_TEST_REQUIREMENT");
    assert.equal(error.message, "native iroha_js_host binding is missing required method(s): absent");
    return true;
  });
});
