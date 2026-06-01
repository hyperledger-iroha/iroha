import assert from "node:assert/strict";
import test from "node:test";

import {
  KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
  KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  isKagemushaRecursiveSpendNativeAvailable,
  kagemushaRecursiveSpendAppend,
  kagemushaRecursiveSpendInit,
  kagemushaRecursiveSpendRedeem,
  kagemushaRecursiveSpendVerify,
  preferredKagemushaOfflineSpendMode,
} from "../src/crypto.js";

function withNativeBinding(binding, fn) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = binding;
  try {
    return fn();
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
}

test("Kagemusha recursive spend helpers reject empty request archives before native calls", () => {
  withNativeBinding({}, () => {
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.alloc(0)),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendAppend(Buffer.alloc(0)),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendVerify(Buffer.alloc(0)),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(Buffer.alloc(0)),
      /requestArchive must not be empty/,
    );
  });
});

test("Kagemusha offline spend mode defaults to recursive when native support is complete", () => {
  const completeBinding = {
    kagemushaRecursiveSpendInit() {},
    kagemushaRecursiveSpendAppend() {},
    kagemushaRecursiveSpendVerify() {},
    kagemushaRecursiveSpendRedeem() {},
  };

  assert.equal(
    preferredKagemushaOfflineSpendMode(true),
    KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  );
  assert.equal(
    preferredKagemushaOfflineSpendMode(false),
    KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
  );
  withNativeBinding(completeBinding, () => {
    assert.equal(
      preferredKagemushaOfflineSpendMode(),
      KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
    );
  });
  withNativeBinding({ kagemushaRecursiveSpendInit() {} }, () => {
    assert.equal(
      preferredKagemushaOfflineSpendMode(),
      KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
    );
  });
});

test("Kagemusha recursive spend helpers probe native availability and return Buffers", () => {
  const calls = [];
  const binding = {
    kagemushaRecursiveSpendInit(request) {
      calls.push(["init", Buffer.from(request)]);
      return Uint8Array.from([1, 2, 3]);
    },
    kagemushaRecursiveSpendAppend(request) {
      calls.push(["append", Buffer.from(request)]);
      return Uint8Array.from([4, 5]);
    },
    kagemushaRecursiveSpendVerify(request) {
      calls.push(["verify", Buffer.from(request)]);
      return Uint8Array.from([6]);
    },
    kagemushaRecursiveSpendRedeem(request) {
      calls.push(["redeem", Buffer.from(request)]);
      return Uint8Array.from([7, 8, 9]);
    },
  };

  withNativeBinding(binding, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), true);
    assert.deepEqual(kagemushaRecursiveSpendInit(Buffer.from([9])), Buffer.from([1, 2, 3]));
    assert.deepEqual(kagemushaRecursiveSpendAppend(Buffer.from([8])), Buffer.from([4, 5]));
    assert.deepEqual(kagemushaRecursiveSpendVerify(Buffer.from([7])), Buffer.from([6]));
    assert.deepEqual(kagemushaRecursiveSpendRedeem(Buffer.from([6])), Buffer.from([7, 8, 9]));
  });

  assert.deepEqual(calls, [
    ["init", Buffer.from([9])],
    ["append", Buffer.from([8])],
    ["verify", Buffer.from([7])],
    ["redeem", Buffer.from([6])],
  ]);
});

test("Kagemusha recursive spend helpers reject empty native outputs", () => {
  const binding = {
    kagemushaRecursiveSpendInit() {
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendAppend() {
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendVerify() {
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendRedeem() {
      return Buffer.alloc(0);
    },
  };

  withNativeBinding(binding, () => {
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.from([1])),
      /native kagemushaRecursiveSpendInit returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendAppend(Buffer.from([1])),
      /native kagemushaRecursiveSpendAppend returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendVerify(Buffer.from([1])),
      /native kagemushaRecursiveSpendVerify returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(Buffer.from([1])),
      /native kagemushaRecursiveSpendRedeem returned empty output/,
    );
  });
});

test("Kagemusha recursive spend helpers reject missing native outputs", () => {
  const binding = {
    kagemushaRecursiveSpendInit() {
      return null;
    },
    kagemushaRecursiveSpendAppend() {
      return undefined;
    },
    kagemushaRecursiveSpendVerify() {
      return null;
    },
    kagemushaRecursiveSpendRedeem() {
      return undefined;
    },
  };

  withNativeBinding(binding, () => {
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.from([1])),
      /native kagemushaRecursiveSpendInit returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendAppend(Buffer.from([1])),
      /native kagemushaRecursiveSpendAppend returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendVerify(Buffer.from([1])),
      /native kagemushaRecursiveSpendVerify returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendRedeem(Buffer.from([1])),
      /native kagemushaRecursiveSpendRedeem returned no output/,
    );
  });
});

test("Kagemusha recursive spend availability fails closed when native methods are partial", () => {
  withNativeBinding({ kagemushaRecursiveSpendInit() {} }, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false);
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.from([1])),
      /Kagemusha recursive spend helper 'kagemushaRecursiveSpendInit' is unavailable/,
    );
  });
});
