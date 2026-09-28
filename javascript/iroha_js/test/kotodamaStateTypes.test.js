import assert from "node:assert/strict";
import test from "node:test";
import { isCanonicalKotodamaStateTypeName } from "../src/kotodamaIdentifiers.js";

test("durable empty products preserve their nominal spelling at root and nested positions", () => {
  for (const type of [
    "Empty{}",
    "Other{}", "Transfer{}",
    "List<Empty{}, 2>",
    "List<List<Empty{}, 2>, 2>",
    "Envelope{empty: Empty{}}",
    "StateMap<int, Empty{}>",
    "std/math@1.0.0::Math::Empty{}",
  ]) assert.equal(isCanonicalKotodamaStateTypeName(type), true, type);
});

test("empty products do not relax canonical syntax or reserved type shapes", () => {
  for (const type of [
    "{}", "Empty{", "Empty{ }", "Empty{,}", "Empty{: int}",
    "Empty{field: int, }", "Empty{}trailing", "List<Empty{},2>",
    "List<Empty{}, 0>", "Envelope{empty: Empty{}, empty: Empty{}}",
    "StatePage{}", "Option{}", "int{}",
  ]) assert.equal(isCanonicalKotodamaStateTypeName(type), false, type);
});
