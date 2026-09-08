import assert from "node:assert/strict";
import test from "node:test";
import { rejectError, rejectRange, rejectType } from "../src/validationThrow.js";

for (const [reject, ErrorClass] of [
  [rejectError, Error], [rejectRange, RangeError], [rejectType, TypeError],
]) {
  test(`${ErrorClass.name} validation boundaries preserve message and cause`, () => {
    const cause = { diagnostic: "original rejection" };
    assert.throws(() => reject("exact validation message", { cause }), (error) => {
      assert.equal(Object.getPrototypeOf(error), ErrorClass.prototype);
      assert.equal(error.message, "exact validation message");
      assert.equal(error.cause, cause);
      assert.equal(Object.hasOwn(error, "cause"), true);
      return true;
    });
    assert.throws(() => reject("without cause"), (error) => {
      assert.equal(Object.hasOwn(error, "cause"), false);
      return true;
    });
  });
}
