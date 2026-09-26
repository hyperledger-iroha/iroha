import test from "node:test";
import assert from "node:assert/strict";

import { SorafsOrderbookSubmissionAmbiguousError as PublicError } from "../src/toriiClient.js";
import { SorafsOrderbookSubmissionAmbiguousError as DirectError } from "../src/sorafsOrderbookSubmission.js";

test("lazy orderbook helpers retain one public ambiguity error identity", async () => {
  const optional = await import("../src/toriiOptional.js");
  assert.strictEqual(PublicError, DirectError);
  assert.equal(typeof optional.prepareSorafsOrderbookSubmission, "function");
  assert.equal(typeof optional.verifySorafsOrderbookSubmissionReceipt, "function");
  const cause = new Error("transport failed after dispatch");
  const error = new PublicError("/v1/sorafs/orderbook", {
    entrypointHash: "a".repeat(64),
    signedTransactionHash: "b".repeat(64),
  }, cause);
  assert.ok(error instanceof DirectError);
  assert.strictEqual(error.cause, cause);
  assert.equal(error.route, "/v1/sorafs/orderbook");
  assert.ok(Object.isFrozen(error.expectedIdentity));
});
