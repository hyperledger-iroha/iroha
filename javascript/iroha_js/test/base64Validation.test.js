import assert from "node:assert/strict";
import test from "node:test";

import { buildRegisterSmartContractBytesInstruction } from "../src/instructionBuilders.js";
import { ValidationErrorCode } from "../src/validationError.js";

test("builders reject invalid base64 payloads", () => {
  const codeHash = Buffer.alloc(32, 0xaa);
  const validCode = Buffer.from("iroha-code-bytes").toString("base64");
  const invalidCode = `${validCode.slice(0, 4)}*${validCode.slice(4)}`;
  assert.throws(
    () =>
      buildRegisterSmartContractBytesInstruction({
        codeHash,
        code: invalidCode,
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_STRING);
      assert.match(String(error?.message), /base64/i);
      return true;
    },
  );
});
