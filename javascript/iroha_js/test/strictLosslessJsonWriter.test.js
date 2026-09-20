import test from "node:test";
import assert from "node:assert/strict";
import {
  parseStrictLosslessIntegerJson,
  stringifyStrictLosslessIntegerJson,
} from "../src/strictLosslessJson.js";

test("strict JSON writer rejects array accessors without invoking them", () => {
  for (const enumerable of [true, false]) {
    let getterCalls = 0;
    const value = [1];
    Object.defineProperty(value, "0", {
      enumerable,
      get() {
        getterCalls += 1;
        value.push(2);
        return 1;
      },
    });
    assert.throws(
      () => stringifyStrictLosslessIntegerJson({ values: value }, "request"),
      /root\.values\[0\]: accessor properties are forbidden/u,
    );
    assert.equal(getterCalls, 0);
    assert.equal(value.length, 1);
  }
});

test("strict JSON writer rejects malformed Unicode in values and keys", () => {
  for (const invalid of ["\ud800", "\udfff", "a\ud800b", "\ud800\ud800", "\udfff\ud800"]) {
    for (const value of [invalid, [invalid], { value: invalid }, { [invalid]: 1 }]) {
      assert.throws(
        () => stringifyStrictLosslessIntegerJson(value, "request"),
        /Unicode scalar/u,
      );
    }
  }
});

test("strict JSON writer roundtrips Unicode scalars, exact integers, and dense arrays", () => {
  const key = "\u{10000}\u{10ffff}";
  const value = Object.assign(Object.create(null), {
    [key]: ["日本語\u0000\ud7ff\ue000\u{1f600}", 9007199254740993n, -7, true, null],
  });
  assert.deepEqual(
    parseStrictLosslessIntegerJson(stringifyStrictLosslessIntegerJson(value, "request"), "response"),
    value,
  );
  assert.throws(() => stringifyStrictLosslessIntegerJson(new Array(1), "request"), /sparse arrays/u);
});
