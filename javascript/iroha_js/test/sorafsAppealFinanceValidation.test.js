import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { validateAppealFinanceCancelAssetLock } from "../src/sorafs.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixtureRoot = path.resolve(
  __dirname,
  "..",
  "..",
  "..",
  "fixtures",
  "sorafs_manifest",
  "appeal_finance",
);

function fixture(relativePath) {
  return fs.readFileSync(path.join(fixtureRoot, relativePath));
}

function expectedProfileText(fileName) {
  return fs.readFileSync(
    path.join(fixtureRoot, "..", "reference_sdk", fileName),
    "utf8",
  );
}

function assertOutcomeShape(outcome, expected) {
  assert.equal(outcome.status, expected.status);
  assert.equal(outcome.code, expected.code);
  assert.equal(outcome.category, expected.category);
  assert.equal(outcome.version, 1);
  assert.equal(outcome.generated_at, expected.generatedAt);
  assert.deepEqual(outcome.inputs, [
    { kind: "cancel_asset_lock", path: expected.label },
  ]);
  assert.equal(
    outcome.telemetry_tags.includes(
      `sorafs.reference.code.${expected.code}`,
    ),
    true,
  );
}

test("appeal-finance validation reports the stable canonical profile", () => {
  const label = "cancel_asset_lock_v1.to";
  const outcome = validateAppealFinanceCancelAssetLock(
    fixture(label),
    { label, generatedAtUnix: 41 },
  );
  assertOutcomeShape(outcome, {
    status: "Ok",
    code: "SFS-OK-000",
    category: "validation",
    generatedAt: 41,
    label,
  });
  assert.equal(outcome.action, null);
  assert.deepEqual(
    outcome.context.find((field) => field.key === "canonical_bytes"),
    { key: "canonical_bytes", value: "85" },
  );
});

test("appeal-finance positive and negative profiles match the signed inventory fixtures", () => {
  for (const [payloadPath, label, expectedFile] of [
    [
      "cancel_asset_lock_v1.to",
      "cancel_asset_lock_v1.to",
      "appeal_finance_cancel_asset_lock_positive_validation_outcome_v1.json",
    ],
    [
      "negative/cancel_asset_lock_zero_expected_v1.to",
      "cancel_asset_lock_zero_expected_v1.to",
      "appeal_finance_cancel_asset_lock_zero_expected_negative_validation_outcome_v1.json",
    ],
  ]) {
    const expectedText = expectedProfileText(expectedFile);
    const outcome = validateAppealFinanceCancelAssetLock(fixture(payloadPath), {
      label,
      generatedAtUnix: 123,
    });
    assert.deepEqual(
      outcome,
      JSON.parse(expectedText),
      payloadPath,
    );
    assert.equal(
      `${JSON.stringify(outcome, null, 2)}\n`,
      expectedText,
      `${payloadPath} deterministic JSON`,
    );
  }
});

test("appeal-finance validation reports the stable missing-field profile", () => {
  const label = "cancel_asset_lock_legacy_missing_expected_v1.to";
  const outcome = validateAppealFinanceCancelAssetLock(
    fixture(`negative/${label}`),
    { label, generatedAtUnix: 42 },
  );
  assertOutcomeShape(outcome, {
    status: "Error",
    code: "SFS-NORITO-001",
    category: "norito",
    generatedAt: 42,
    label,
  });
  assert.equal(typeof outcome.action, "string");
  assert.match(outcome.message, /failed to decode CancelAssetLock/u);
});

test("appeal-finance validation reports the stable zero-quantity profile", () => {
  const label = "cancel_asset_lock_zero_expected_v1.to";
  const outcome = validateAppealFinanceCancelAssetLock(
    fixture(`negative/${label}`),
    { label, generatedAtUnix: 43 },
  );
  assertOutcomeShape(outcome, {
    status: "Error",
    code: "SFS-VAL-001",
    category: "validation",
    generatedAt: 43,
    label,
  });
  assert.equal(typeof outcome.action, "string");
  assert.match(outcome.message, /greater than zero/u);
});

test("appeal-finance validation wrapper rejects unsafe boundary inputs", () => {
  const canonical = fixture("cancel_asset_lock_v1.to");
  for (const bytes of [
    canonical.toString("hex"),
    canonical.toString("base64"),
    [...canonical],
  ]) {
    assert.throws(() => validateAppealFinanceCancelAssetLock(bytes));
  }
  assert.throws(
    () =>
      validateAppealFinanceCancelAssetLock(canonical, {
        generatedAtUnix: Number.MAX_SAFE_INTEGER + 1,
      }),
    /safe integer/u,
  );
  assert.throws(
    () =>
      validateAppealFinanceCancelAssetLock(canonical, {
        generated_at: 1,
      }),
    /unsupported fields/u,
  );
});
