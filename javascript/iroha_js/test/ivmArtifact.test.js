import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";

import {
  computeIvmArtifactHashes,
  IVM_ARTIFACT_MAX_BYTES,
  IVM_PROGRAM_HEADER_LENGTH,
} from "../src/ivmArtifact.js";

const ARTIFACT = Uint8Array.from([
  0x49, 0x56, 0x4d, 0x00,
  0x01, 0x01, 0x01, 0x00,
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  0x01,
  ...Array(32).fill(0),
]);

test("computeIvmArtifactHashes matches ledger full-artifact fixtures", () => {
  assert.equal(IVM_PROGRAM_HEADER_LENGTH, 49);
  assert.equal(IVM_ARTIFACT_MAX_BYTES, 4 * 1024 * 1024);
  assert.deepEqual(computeIvmArtifactHashes(ARTIFACT), {
    codeHashHex:
      "b5d6d7f7abf5989ca07b4fbee75560ab7a3dbceaafd442da66a6918e3cb147d1",
    artifactSha256Hex:
      "b004dd0c3eddd8e1c729e18ce88a2c6ab225fc21f3be1ccf55bac71d403826e6",
  });
});

test("computeIvmArtifactHashes distinguishes header and body substitution", () => {
  const original = computeIvmArtifactHashes(ARTIFACT);
  const changedHeader = ARTIFACT.slice();
  changedHeader[16] ^= 0x80;
  const headerHashes = computeIvmArtifactHashes(changedHeader);
  assert.notEqual(headerHashes.codeHashHex, original.codeHashHex);
  assert.notEqual(headerHashes.artifactSha256Hex, original.artifactSha256Hex);

  const changedBody = Uint8Array.from([...ARTIFACT, 0x80]);
  const bodyHashes = computeIvmArtifactHashes(changedBody);
  assert.notEqual(bodyHashes.codeHashHex, original.codeHashHex);
  assert.notEqual(bodyHashes.artifactSha256Hex, original.artifactSha256Hex);
});

test("computeIvmArtifactHashes rejects ambiguous or malformed binary inputs", () => {
  assert.throws(
    () => computeIvmArtifactHashes("SVZNAA=="),
    /Uint8Array, ArrayBuffer, or ArrayBuffer view/,
  );
  assert.throws(
    () => computeIvmArtifactHashes(ARTIFACT.subarray(0, 17)),
    /at least the 49-byte program header/,
  );
  assert.throws(
    () => computeIvmArtifactHashes(ARTIFACT.subarray(0, 48)),
    /at least the 49-byte program header/,
  );
  const badMagic = ARTIFACT.slice();
  badMagic[0] ^= 0xff;
  assert.throws(
    () => computeIvmArtifactHashes(badMagic.buffer),
    /invalid program header magic/,
  );
});

test("computeIvmArtifactHashes bounds input before copying and rejects shared memory", () => {
  const oversized = new Uint8Array(IVM_ARTIFACT_MAX_BYTES + 1);
  oversized.set(ARTIFACT);
  assert.throws(
    () => computeIvmArtifactHashes(oversized),
    /exceeds the 4194304-byte limit/,
  );

  if (typeof SharedArrayBuffer === "function") {
    const shared = new SharedArrayBuffer(ARTIFACT.byteLength);
    new Uint8Array(shared).set(ARTIFACT);
    assert.throws(
      () => computeIvmArtifactHashes(new Uint8Array(shared)),
      /must not be backed by SharedArrayBuffer/,
    );
    assert.throws(
      () => computeIvmArtifactHashes(new DataView(shared)),
      /must not be backed by SharedArrayBuffer/,
    );

    const foreignView = vm.runInNewContext(
      `(() => {
        const bytes = new Uint8Array(new SharedArrayBuffer(${ARTIFACT.byteLength}));
        bytes.set([${[...ARTIFACT].join(",")}]);
        return bytes;
      })()`,
    );
    assert.equal(foreignView instanceof Uint8Array, false);
    assert.equal(ArrayBuffer.isView(foreignView), true);
    assert.throws(
      () => computeIvmArtifactHashes(foreignView),
      /must not be backed by SharedArrayBuffer/,
    );

    const foreignDataView = vm.runInNewContext(
      `new DataView(new SharedArrayBuffer(${ARTIFACT.byteLength}))`,
    );
    Object.defineProperty(foreignDataView, "buffer", {
      value: ARTIFACT.buffer,
    });
    assert.throws(
      () => computeIvmArtifactHashes(foreignDataView),
      /must not be backed by SharedArrayBuffer/,
    );

    const foreignSharedBuffer = vm.runInNewContext(
      `new SharedArrayBuffer(${ARTIFACT.byteLength})`,
    );
    assert.throws(
      () => computeIvmArtifactHashes(foreignSharedBuffer),
      /must not be backed by SharedArrayBuffer/,
    );
  }

  const foreignBuffer = vm.runInNewContext(
    `Uint8Array.from([${[...ARTIFACT].join(",")}]).buffer`,
  );
  assert.equal(foreignBuffer instanceof ArrayBuffer, false);
  Object.defineProperties(foreignBuffer, {
    byteLength: {
      get() {
        throw new Error("shadow byteLength must not be read");
      },
    },
    slice: {
      value() {
        throw new Error("shadow slice must not be invoked");
      },
    },
  });
  assert.deepEqual(
    computeIvmArtifactHashes(foreignBuffer),
    computeIvmArtifactHashes(ARTIFACT),
  );

  for (const kind of ["Uint8Array", "DataView"]) {
    const foreignView = vm.runInNewContext(
      `(() => {
        const bytes = new Uint8Array(${ARTIFACT.byteLength + 6});
        bytes.set([${[...ARTIFACT].join(",")}], 3);
        return ${kind === "Uint8Array"
          ? `new Uint8Array(bytes.buffer, 3, ${ARTIFACT.byteLength})`
          : `new DataView(bytes.buffer, 3, ${ARTIFACT.byteLength})`};
      })()`,
    );
    Object.defineProperties(foreignView, {
      buffer: {
        get() {
          throw new Error("shadow buffer must not be read");
        },
      },
      byteOffset: {
        get() {
          throw new Error("shadow byteOffset must not be read");
        },
      },
      byteLength: {
        get() {
          throw new Error("shadow byteLength must not be read");
        },
      },
    });
    assert.deepEqual(
      computeIvmArtifactHashes(foreignView),
      computeIvmArtifactHashes(ARTIFACT),
    );
  }

  if (typeof SharedArrayBuffer === "function") {
    for (const sharedView of [
      new Uint8Array(new SharedArrayBuffer(ARTIFACT.byteLength)),
      new DataView(new SharedArrayBuffer(ARTIFACT.byteLength)),
    ]) {
      Object.defineProperties(sharedView, {
        buffer: { value: ARTIFACT.buffer },
        byteOffset: { value: 0 },
        byteLength: { value: ARTIFACT.byteLength },
      });
      assert.throws(
        () => computeIvmArtifactHashes(sharedView),
        /must not be backed by SharedArrayBuffer/,
      );
    }
  }

  for (const tag of ["ArrayBuffer", "SharedArrayBuffer"]) {
    const spoof = {
      byteLength: ARTIFACT.byteLength,
      slice() {
        throw new Error("spoof slice must not be invoked");
      },
      [Symbol.toStringTag]: tag,
    };
    assert.throws(
      () => computeIvmArtifactHashes(spoof),
      /Uint8Array, ArrayBuffer, or ArrayBuffer view/,
    );
  }
});

test("computeIvmArtifactHashes copies without consulting ArrayBuffer species", () => {
  const expected = computeIvmArtifactHashes(ARTIFACT);
  for (const asView of [false, true]) {
    const buffer = ARTIFACT.slice().buffer;
    let constructorReads = 0;
    let speciesReads = 0;
    const hostileConstructor = {};
    Object.defineProperty(hostileConstructor, Symbol.species, {
      get() {
        speciesReads += 1;
        throw new Error("hostile species must not run");
      },
    });
    Object.defineProperty(buffer, "constructor", {
      configurable: true,
      get() {
        constructorReads += 1;
        return hostileConstructor;
      },
    });
    const input = asView ? new Uint8Array(buffer) : buffer;
    assert.deepEqual(computeIvmArtifactHashes(input), expected);
    assert.equal(constructorReads, 0);
    assert.equal(speciesReads, 0);
  }

  const foreignBuffer = vm.runInNewContext(
    `Uint8Array.from([${[...ARTIFACT].join(",")}]).buffer`,
  );
  let foreignConstructorReads = 0;
  Object.defineProperty(foreignBuffer, "constructor", {
    get() {
      foreignConstructorReads += 1;
      throw new Error("cross-realm constructor must not run");
    },
  });
  assert.deepEqual(computeIvmArtifactHashes(foreignBuffer), expected);
  assert.equal(foreignConstructorReads, 0);
});
