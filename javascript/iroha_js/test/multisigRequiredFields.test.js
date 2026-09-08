import assert from "node:assert/strict";
import test from "node:test";

import { isMultisigSignerAuthorized } from "../src/instructionBuilders.js";
import { ValidationError, ValidationErrorCode } from "../src/validationError.js";

const cases = [
  ["absent quorum", {}, "quorum", "is required", ValidationErrorCode.INVALID_NUMERIC],
  ["null quorum", { quorum: null }, "quorum", "is required", ValidationErrorCode.INVALID_NUMERIC],
  ["absent TTL", { quorum: 1 }, "transaction_ttl_ms", "is required", ValidationErrorCode.INVALID_NUMERIC],
  ["null TTL", { quorum: 1, transaction_ttl_ms: null }, "transaction_ttl_ms", "is required", ValidationErrorCode.INVALID_NUMERIC],
  ["empty signatories", { quorum: 1, transaction_ttl_ms: 1, signatories: {} }, "signatories", "must contain at least one entry", ValidationErrorCode.INVALID_OBJECT],
  ["empty alias signatories", { quorumRaw: 1, transactionTtlMs: 1, members: Object.create(null) }, "signatories", "must contain at least one entry", ValidationErrorCode.INVALID_OBJECT],
];

for (const [label, spec, field, diagnostic, code] of cases) {
  test(`multisig ${label} has the canonical diagnostic before signer admission`, () => {
    const signer = new Proxy({}, {
      get() { assert.fail("invalid specifications must be rejected before inspecting the signer"); },
    });
    assert.throws(() => isMultisigSignerAuthorized(spec, signer), (error) => {
      assert.ok(error instanceof ValidationError);
      assert.ok(error instanceof TypeError);
      assert.equal(error.name, "ValidationError");
      assert.equal(error.code, code);
      assert.equal(error.path, `spec.${field}`);
      assert.equal(error.message, `spec.${field} ${diagnostic}`);
      assert.equal(Object.hasOwn(error, "cause"), false);
      return true;
    });
  });
}
