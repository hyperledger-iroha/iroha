import assert from "node:assert/strict";
import test from "node:test";

import {
  KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
  KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
  KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION,
  KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
  KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
  isKagemushaRecursiveSpendNativeAvailable,
  kagemushaRecursiveSpendAppend,
  kagemushaRecursiveSpendInit,
  kagemushaRecursiveSpendLineageWitnessAppendResult,
  kagemushaRecursiveSpendLineageWitnessFromInitResult,
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
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.alloc(0), Buffer.from([1])),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.from([1]), Buffer.alloc(0)),
      /bundleArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.alloc(0), Buffer.from([1]), Buffer.from([2])),
      /previousWitnessArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.from([1]), Buffer.alloc(0), Buffer.from([2])),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.from([1]), Buffer.from([2]), Buffer.alloc(0)),
      /bundleArchive must not be empty/,
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
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    kagemushaRecursiveSpendInit() {},
    kagemushaRecursiveSpendAppend() {},
    kagemushaRecursiveSpendLineageWitnessFromInitResult() {},
    kagemushaRecursiveSpendLineageWitnessAppendResult() {},
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

test("Kagemusha recursive spend exports stable proof circuit ids", () => {
  assert.equal(KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION, 6);
  assert.equal(
    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    "kagemusha-recursive-aggregation-v1",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    "kagemusha-recursive-spend-lineage-v1",
  );
});

test("Kagemusha recursive spend helpers probe native availability and return Buffers", () => {
  const calls = [];
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    kagemushaRecursiveSpendInit(request) {
      calls.push(["init", Buffer.from(request)]);
      return Uint8Array.from([1, 2, 3]);
    },
    kagemushaRecursiveSpendAppend(request) {
      calls.push(["append", Buffer.from(request)]);
      return Uint8Array.from([4, 5]);
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult(request, bundle) {
      calls.push(["lineage-init", Buffer.from(request), Buffer.from(bundle)]);
      return Uint8Array.from([10, 11]);
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult(previousWitness, request, bundle) {
      calls.push([
        "lineage-append",
        Buffer.from(previousWitness),
        Buffer.from(request),
        Buffer.from(bundle),
      ]);
      return Uint8Array.from([12, 13]);
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
    assert.deepEqual(
      kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.from([3]), Buffer.from([4])),
      Buffer.from([10, 11]),
    );
    assert.deepEqual(
      kagemushaRecursiveSpendLineageWitnessAppendResult(
        Buffer.from([5]),
        Buffer.from([6]),
        Buffer.from([7]),
      ),
      Buffer.from([12, 13]),
    );
    assert.deepEqual(kagemushaRecursiveSpendVerify(Buffer.from([7])), Buffer.from([6]));
    assert.deepEqual(kagemushaRecursiveSpendRedeem(Buffer.from([6])), Buffer.from([7, 8, 9]));
  });

  assert.deepEqual(calls, [
    ["init", Buffer.from([9])],
    ["append", Buffer.from([8])],
    ["lineage-init", Buffer.from([3]), Buffer.from([4])],
    ["lineage-append", Buffer.from([5]), Buffer.from([6]), Buffer.from([7])],
    ["verify", Buffer.from([7])],
    ["redeem", Buffer.from([6])],
  ]);
});

test("Kagemusha recursive spend availability requires bridge ABI 6", () => {
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return 5;
    },
    kagemushaRecursiveSpendInit() {},
    kagemushaRecursiveSpendAppend() {},
    kagemushaRecursiveSpendLineageWitnessFromInitResult() {},
    kagemushaRecursiveSpendLineageWitnessAppendResult() {},
    kagemushaRecursiveSpendVerify() {},
    kagemushaRecursiveSpendRedeem() {},
  };

  withNativeBinding(binding, () => {
    assert.equal(isKagemushaRecursiveSpendNativeAvailable(), false);
    assert.equal(
      preferredKagemushaOfflineSpendMode(),
      KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1,
    );
    assert.throws(
      () => kagemushaRecursiveSpendInit(Buffer.from([1])),
      /Kagemusha recursive spend helper 'kagemushaRecursiveSpendInit' is unavailable/,
    );
  });
});

test("Kagemusha recursive spend helpers reject empty native outputs", () => {
  const binding = {
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    kagemushaRecursiveSpendInit() {
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendAppend() {
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult() {
      return Buffer.alloc(0);
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult() {
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
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.from([1]), Buffer.from([2])),
      /native kagemushaRecursiveSpendLineageWitnessFromInitResult returned empty output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.from([1]), Buffer.from([2]), Buffer.from([3])),
      /native kagemushaRecursiveSpendLineageWitnessAppendResult returned empty output/,
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
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    kagemushaRecursiveSpendInit() {
      return null;
    },
    kagemushaRecursiveSpendAppend() {
      return undefined;
    },
    kagemushaRecursiveSpendLineageWitnessFromInitResult() {
      return null;
    },
    kagemushaRecursiveSpendLineageWitnessAppendResult() {
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
      () => kagemushaRecursiveSpendLineageWitnessFromInitResult(Buffer.from([1]), Buffer.from([2])),
      /native kagemushaRecursiveSpendLineageWitnessFromInitResult returned no output/,
    );
    assert.throws(
      () => kagemushaRecursiveSpendLineageWitnessAppendResult(Buffer.from([1]), Buffer.from([2]), Buffer.from([3])),
      /native kagemushaRecursiveSpendLineageWitnessAppendResult returned no output/,
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
