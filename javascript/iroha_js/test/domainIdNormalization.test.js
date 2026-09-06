"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import { canonicalizeDomainIdLabel } from "../src/domainId.js";
import { _createNoritoInstructionApi } from "../src/norito.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";

const { noritoEncodeInstruction } = _createNoritoInstructionApi(
  createNativeRuntime({
    noritoEncodeInstruction() {
      throw new Error("unsupported instruction");
    },
  }),
);

function registerDomain(domainId) {
  return {
    Register: {
      Domain: {
        id: domainId,
        logo: null,
        metadata: {},
      },
    },
  };
}

test("internal DomainId label normalization retains explicit-domain policy", () => {
  for (const canonical of [
    "xn--exmple-cua",
    "xn--fa-hia",
    "xn--3xa",
    "xn--ll-0ea",
    "xn--mgbh0fb",
    "xn--ngba799q",
    "xn--ngba7iz95i",
    "xn--11b2ezcw70k",
    "xn--mgba3gch31f060k",
    "xn--ab-0ea",
    "xn--a-jib",
    "xn--ab-3n4a",
    "foo_bar",
  ]) {
    assert.equal(canonicalizeDomainIdLabel(canonical), canonical);
  }
  assert.equal(canonicalizeDomainIdLabel("BÜCHER"), "xn--bcher-kva");

  for (const invalid of [
    "-leading",
    "trailing-",
    "ab--cd",
    "ḷ",
    "foo:123",
    "foo/bar",
    "foo\\bar",
    "foo?bar",
    "foo%41",
    " foo",
    "foo ",
    "\uFEFFfoo",
    "foo\uFEFF",
    "xn--",
    "xn--a",
    "xn--alice",
    "xn--ab-uuba211bca8057b",
    "xn--1-zmcl5hc",
    "xn--a-zmck6hb",
    "xn--1-ymcl5hc6o",
    "xn--_-ymcl5hc",
    "xn--ab-j1t",
    "xn--11b2er09f",
  ]) {
    assert.throws(() => canonicalizeDomainIdLabel(invalid), TypeError, invalid);
  }
});

test("pure-JS DomainId encoding canonicalizes labels without AccountAddress", () => {
  const encode = (domainId) =>
    Buffer.from(noritoEncodeInstruction(registerDomain(domainId)));

  assert.deepEqual(
    encode("BÜCHER.SORA"),
    encode("xn--bcher-kva.sora"),
  );
  assert.throws(() => encode("bad@name.sora"), TypeError);
});
