import { test } from "node:test";
import assert from "node:assert/strict";

import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
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

test("normalizeAccountId canonicalizes domain labels with UTS-46 rules", () => {
  const normalized = normalizeAccountId("alice@Exämple", "accountId");
  assert.equal(normalized, "alice@xn--exmple-cua");
});

test("normalizeAccountId accepts default-domain literal without suffix", () => {
  const address = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
    publicKey: SAMPLE_KEY,
  });
  const literalWithoutDomain = address.toIH58();
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
      /append '@domain'/i.test(error.message),
  );
});

const maybeTestCompressed = process.env.IROHA_JS_DISABLE_NATIVE === "1" ? test.skip : test;

maybeTestCompressed("normalizeAccountId accepts compressed default-domain literal without suffix", () => {
  const address = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
    publicKey: SAMPLE_KEY,
  });
  const compressed = address.toCompressedSora();
  const normalized = normalizeAccountId(compressed, "accountId");

  const parsed = AccountAddress.parseAny(normalized, undefined, DEFAULT_DOMAIN_NAME).address;
  assert.deepEqual(
    Buffer.from(parsed.canonicalBytes()),
    Buffer.from(address.canonicalBytes()),
  );
});

test("normalizeAccountId canonicalizes compressed literal without suffix for non-default domain", () => {
  const address = AccountAddress.fromAccount({
    domain: "wonderland",
    publicKey: SAMPLE_KEY,
  });
  const compressed = address.toCompressedSora();

  const normalized = normalizeAccountId(compressed, "accountId");
  const parsed = AccountAddress.parseAny(normalized, undefined, "wonderland").address;
  assert.deepEqual(
    Buffer.from(parsed.canonicalBytes()),
    Buffer.from(address.canonicalBytes()),
  );
});

test("normalizeAccountId canonicalizes non-default IH58 literal without suffix", () => {
  const address = AccountAddress.fromAccount({
    domain: "wonderland",
    publicKey: SAMPLE_KEY,
  });
  const literalWithoutDomain = address.toIH58();
  const normalized = normalizeAccountId(literalWithoutDomain, "accountId");
  assert.equal(normalized, literalWithoutDomain);
});

test("normalizeAccountId rejects invalid domain labels", () => {
  assert.throws(
    () => normalizeAccountId("alice@bad domain", "accountId"),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_ACCOUNT_ID &&
      error.cause instanceof AccountAddressError &&
      error.cause.code === AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
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
  const mismatchedId = `${address.toIH58()}@${DEFAULT_DOMAIN_NAME}`;
  assert.throws(
    () => normalizeAccountId(mismatchedId, "accountId"),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_ACCOUNT_ID &&
      /append '@domain'/i.test(error.message),
  );
});

test("normalizeAccountId bypasses canonical parsing for friendly identifiers", () => {
  const originalParseAny = AccountAddress.parseAny;
  let parseCalls = 0;
  AccountAddress.parseAny = () => {
    parseCalls += 1;
    throw new AccountAddressError(
      AccountAddressErrorCode.INVALID_IH58_ENCODING,
      "invalid IH58 literal",
    );
  };
  try {
    const normalized = normalizeAccountId("carol@legacy-domain", "accountId");
    assert.equal(normalized, "carol@legacy-domain");
    assert.ok(parseCalls >= 1, "fallback path should attempt canonical parsing before accepting literal");
  } finally {
    AccountAddress.parseAny = originalParseAny;
  }
});
