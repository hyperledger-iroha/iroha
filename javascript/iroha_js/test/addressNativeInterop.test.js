"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";

import {
  AccountAddress,
  __resetAddressNativeStateForTests,
} from "../src/address.js";

const SAMPLE_PUBLIC_KEY = Uint8Array.from([
  0x64, 0x12, 0x97, 0x07, 0x93, 0x57, 0x22, 0x9f,
  0x29, 0x59, 0x38, 0xa4, 0xb5, 0xa3, 0x33, 0xde,
  0x35, 0x06, 0x9b, 0xf4, 0x7b, 0x9d, 0x07, 0x04,
  0xe4, 0x58, 0x05, 0x71, 0x3d, 0x13, 0xc2, 0x01,
]);

test("AccountAddress lazily resolves injected native codecs after import", () => {
  const canonical = AccountAddress.fromAccount({
    domain: "wonderland",
    publicKey: SAMPLE_PUBLIC_KEY,
  }).canonicalBytes();
  let parseCalls = 0;
  let renderCalls = 0;

  try {
    globalThis.__IROHA_NATIVE_BINDING__ = {
      accountAddressParseEncoded(input, expectedPrefix) {
        parseCalls += 1;
        assert.equal(input, "synthetic-i105");
        assert.equal(expectedPrefix, 753);
        return {
          canonical_bytes: canonical,
          network_prefix: 753,
        };
      },
      accountAddressRender(bytes, prefix) {
        renderCalls += 1;
        assert.deepEqual(Buffer.from(bytes), Buffer.from(canonical));
        assert.equal(prefix, 753);
        return {
          canonical_hex: "0xsynthetic",
          i105: "synthetic-i105",
          i105_default: "synthetic-i105-default",
        };
      },
    };
    __resetAddressNativeStateForTests();

    const parsed = AccountAddress.parseEncoded("synthetic-i105", 753);
    assert.equal(parsed.chainDiscriminant, undefined);
    assert.deepEqual(
      Buffer.from(parsed.address.canonicalBytes()),
      Buffer.from(canonical),
    );
    assert.equal(parsed.address.toI105(753), "synthetic-i105");
    assert.equal(parseCalls, 1);
    assert.equal(renderCalls, 1);
  } finally {
    delete globalThis.__IROHA_NATIVE_BINDING__;
    __resetAddressNativeStateForTests();
  }
});
