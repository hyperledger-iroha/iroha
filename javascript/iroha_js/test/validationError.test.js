import { test } from "node:test";
import assert from "node:assert/strict";

import {
  AccountAddress,
  DEFAULT_DOMAIN_NAME,
  ValidationError,
  ValidationErrorCode,
  normalizeAccountId,
  buildRegisterDomainInstruction,
} from "../src/index.js";

const SAMPLE_KEY = Buffer.from(
  "B935AAF1F4E44B3DB79E5E5A9BA4569E6F3E2310C219F3DDD56D3277828D5480",
  "hex",
);

test("normalizeAccountId exposes ValidationError metadata", () => {
  assert.throws(
    () => normalizeAccountId("missing-domain", "accountId"),
    (error) => {
      assert.ok(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_ACCOUNT_ID);
      assert.equal(error.path, "accountId");
      return true;
    },
  );
});

test("normalizeAccountId rejects '@domain' literals (no UTS-46 canonicalization path)", () => {
  assert.throws(
    () => normalizeAccountId("alice@Exämple", "accountId"),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_ACCOUNT_ID &&
      /must not include '@domain'/i.test(error.message),
  );
});

test("normalizeAccountId accepts default-domain literal without suffix", () => {
  const address = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
    publicKey: SAMPLE_KEY,
  });
  const literalWithoutDomain = address.toI105();
  const normalized = normalizeAccountId(literalWithoutDomain, "accountId");
  assert.equal(normalized, literalWithoutDomain);
  assert.throws(
    () =>
      normalizeAccountId(
        `${literalWithoutDomain}@${DEFAULT_DOMAIN_NAME}`,
        "accountId",
      ),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_ACCOUNT_ID &&
      /must not include '@domain'/i.test(error.message),
  );
});

test("normalizeAccountId rejects canonical hex account literals", () => {
  const address = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
    publicKey: SAMPLE_KEY,
  });
  const canonicalHex = address.canonicalHex();
  assert.throws(
    () => normalizeAccountId(canonicalHex, "accountId"),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_ACCOUNT_ID &&
      /must be an I105 account id/i.test(error.message),
  );
});

const maybeTestI105Default = process.env.IROHA_JS_DISABLE_NATIVE === "1" ? test.skip : test;

maybeTestI105Default("normalizeAccountId accepts i105Default default-domain literal without suffix", () => {
  const address = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
    publicKey: SAMPLE_KEY,
  });
  const i105Default = address.toI105Default();
  const normalized = normalizeAccountId(i105Default, "accountId");

  const parsed = AccountAddress.parseEncoded(normalized, undefined, DEFAULT_DOMAIN_NAME).address;
  assert.deepEqual(
    Buffer.from(parsed.canonicalBytes()),
    Buffer.from(address.canonicalBytes()),
  );
});

test("normalizeAccountId canonicalizes i105Default literal without suffix for non-default domain", () => {
  const address = AccountAddress.fromAccount({
    domain: "wonderland",
    publicKey: SAMPLE_KEY,
  });
  const i105Default = address.toI105Default();

  const normalized = normalizeAccountId(i105Default, "accountId");
  const parsed = AccountAddress.parseEncoded(normalized, undefined, "wonderland").address;
  assert.deepEqual(
    Buffer.from(parsed.canonicalBytes()),
    Buffer.from(address.canonicalBytes()),
  );
});

test("normalizeAccountId canonicalizes non-default I105 literal without suffix", () => {
  const address = AccountAddress.fromAccount({
    domain: "wonderland",
    publicKey: SAMPLE_KEY,
  });
  const literalWithoutDomain = address.toI105();
  const normalized = normalizeAccountId(literalWithoutDomain, "accountId");
  assert.equal(normalized, literalWithoutDomain);
});

test("normalizeAccountId rejects invalid domain labels", () => {
  assert.throws(
    () => normalizeAccountId("alice@bad domain", "accountId"),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_ACCOUNT_ID &&
      /must not include '@domain'/i.test(error.message),
  );
});

test("instruction builders propagate ValidationError", () => {
  assert.throws(
    () =>
      buildRegisterDomainInstruction({
        domainId: "",
      }),
    (error) => {
      assert.ok(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
      assert.equal(error.path, "domainId");
      return true;
    },
  );
});

test("ValidationError preserves metadata", () => {
  const cause = new Error("inner");
  const error = new ValidationError(ValidationErrorCode.INVALID_STRING, "bad id", {
    path: "accountId",
    cause,
  });

  assert.ok(error instanceof Error);
  assert.ok(error instanceof TypeError);
  assert.equal(error.name, "ValidationError");
  assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
  assert.equal(error.path, "accountId");
  assert.equal(error.cause, cause);
});

test("normalizeAccountId rejects encoded addresses with domain suffixes", () => {
  const address = AccountAddress.fromAccount({
    domain: "wonderland",
    publicKey: SAMPLE_KEY,
  });
  const mismatchedId = `${address.toI105()}@${DEFAULT_DOMAIN_NAME}`;
  assert.throws(
    () => normalizeAccountId(mismatchedId, "accountId"),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_ACCOUNT_ID &&
      /must not include '@domain'/i.test(error.message),
  );
});

test("normalizeAccountId rejects alias-style literals with domain suffixes", () => {
  const aliasLike = `ed0120${"ab".repeat(32)}`;
  assert.throws(
    () => normalizeAccountId(`${aliasLike}@Example-Domain`, "accountId"),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_ACCOUNT_ID &&
      /must not include '@domain'/i.test(error.message),
  );
});
