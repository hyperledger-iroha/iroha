"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import * as packageExports from "../dist/index.js";

const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);

const {
  PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
  PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
  isPrivacyNativeAvailable,
  privacyBuildProofV1,
  privacyCapabilitiesV1,
  privacyProofRequestV1,
  privacyVerifyProofV1,
} = packageExports;

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

function privacyNoritoFrame(schemaByte) {
  const frame = Buffer.alloc(40);
  frame.write("NRT0", 0, "ascii");
  frame.fill(schemaByte, 6, 22);
  return frame;
}

function privacyNoritoFrameWithPayload(schemaByte) {
  const frame = Buffer.concat([
    privacyNoritoFrame(schemaByte),
    Buffer.from([0x00, 0x00, 0xa5, 0x5a, 0x11]),
  ]);
  frame.writeBigUInt64LE(3n, 23);
  Buffer.from([0xb9, 0xd3, 0xa8, 0x0c, 0xcd, 0x5d, 0x13, 0x24]).copy(frame, 31);
  return frame;
}

function privacyNoritoFrameWithPadding(schemaByte, paddingLength) {
  const frame = Buffer.concat([
    privacyNoritoFrame(schemaByte),
    Buffer.alloc(paddingLength),
    Buffer.from([0xa5, 0x5a, 0x11]),
  ]);
  frame.writeBigUInt64LE(3n, 23);
  Buffer.from([0xb9, 0xd3, 0xa8, 0x0c, 0xcd, 0x5d, 0x13, 0x24]).copy(frame, 31);
  return frame;
}

function privacyNoritoFrameWithSchemaOverride(schemaByte, offset, value) {
  const frame = Buffer.from(privacyNoritoFrameWithPayload(schemaByte));
  frame[offset] = value;
  return frame;
}

function privacyNoritoFrameWithDeclaredPayloadLength(schemaByte, payloadLength) {
  const frame = Buffer.from(privacyNoritoFrameWithPayload(schemaByte));
  frame.writeBigUInt64LE(BigInt(payloadLength), 23);
  return frame;
}

function privacyNoritoFrameWithFlags(schemaByte, flags) {
  const frame = Buffer.from(privacyNoritoFrameWithPayload(schemaByte));
  frame[39] = flags;
  return frame;
}

function slicedPrivacyView(
  archive,
  prefix = [0xff, 0x7f, 0x42],
  suffix = [0x24, 0x13],
) {
  const backing = Uint8Array.from([...prefix, ...archive, ...suffix]);
  return backing.subarray(prefix.length, prefix.length + archive.length);
}

function malformedPrivacyRequestArchives() {
  const badMagic = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badMagic[0] = 0x00;
  const badVersion = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badVersion[4] = 1;
  const badMinorVersion = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badMinorVersion[5] = 1;
  const badCompression = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badCompression[22] = 1;
  const badDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(0x52, 6n);
  const badOversizedDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(
    0x52,
    0x8000000000000000n,
  );
  const badPadding = Buffer.concat([PRIVACY_REQUEST_ARCHIVE, Buffer.from([0x7f])]);
  const badExcessivePadding = privacyNoritoFrameWithPadding(0x52, 65);
  const badFlags = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badFlags[39] = 0x08;
  const badFieldBitsetFlags = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badFieldBitsetFlags[39] = 0x20;
  const badChecksum = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badChecksum[31] ^= 0x01;
  const badPayload = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  badPayload[44] ^= 0x7f;
  return [
    Buffer.from([1]),
    badMagic,
    badVersion,
    badMinorVersion,
    badCompression,
    badDeclaredPayloadLength,
    badOversizedDeclaredPayloadLength,
    badPadding,
    badExcessivePadding,
    badFlags,
    badFieldBitsetFlags,
    badChecksum,
    badPayload,
  ];
}

const PRIVACY_CAPABILITIES_ARCHIVE = privacyNoritoFrameWithPayload(0x50);
const PRIVACY_BUILD_ARCHIVE = privacyNoritoFrameWithPayload(0x42);
const PRIVACY_VERIFY_ARCHIVE = privacyNoritoFrameWithPayload(0x56);
const PRIVACY_REQUEST_ARCHIVE = privacyNoritoFrameWithPayload(0x52);

function malformedPrivacyNativeOutputArchives(schemaByte) {
  const archive = privacyNoritoFrameWithPayload(schemaByte);
  const badMagic = Buffer.from(archive);
  badMagic[0] = 0x00;
  const badVersion = Buffer.from(archive);
  badVersion[4] = 1;
  const badMinorVersion = Buffer.from(archive);
  badMinorVersion[5] = 1;
  const badCompression = Buffer.from(archive);
  badCompression[22] = 1;
  const badDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(
    schemaByte,
    6n,
  );
  const badOversizedDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(
    schemaByte,
    0x8000000000000000n,
  );
  const badPadding = Buffer.concat([archive, Buffer.from([0x7f])]);
  const badExcessivePadding = privacyNoritoFrameWithPadding(schemaByte, 65);
  const badFlags = Buffer.from(archive);
  badFlags[39] = 0x08;
  const badFieldBitsetFlags = Buffer.from(archive);
  badFieldBitsetFlags[39] = 0x20;
  const badChecksum = Buffer.from(archive);
  badChecksum[31] ^= 0x01;
  const badPayload = Buffer.from(archive);
  badPayload[44] ^= 0x7f;
  return [
    Buffer.from([1]),
    badMagic,
    badVersion,
    badMinorVersion,
    badCompression,
    badDeclaredPayloadLength,
    badOversizedDeclaredPayloadLength,
    badPadding,
    badExcessivePadding,
    badFlags,
    badFieldBitsetFlags,
    badChecksum,
    badPayload,
  ];
}

function wrongSchemaPrivacyRequestArchives() {
  return [
    PRIVACY_CAPABILITIES_ARCHIVE,
    PRIVACY_BUILD_ARCHIVE,
    PRIVACY_VERIFY_ARCHIVE,
    privacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42),
    privacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56),
  ];
}

function completePrivacyBinding(overrides = {}) {
  return {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(PRIVACY_CAPABILITIES_ARCHIVE);
    },
    privacyProofRequestV1(
      algorithmId,
      entrypoint,
      vkRef,
      publicInputs,
      witness,
      proof,
    ) {
      assert.equal(typeof algorithmId, "string");
      assert.equal(typeof entrypoint, "string");
      assert.equal(typeof vkRef, "string");
      assert.ok(Buffer.from(publicInputs).length > 0);
      assert.ok(Buffer.isBuffer(publicInputs));
      assert.ok(Buffer.isBuffer(witness));
      assert.ok(Buffer.isBuffer(proof));
      return Uint8Array.from(PRIVACY_REQUEST_ARCHIVE);
    },
    privacyBuildProofV1(request) {
      assert.ok(Buffer.from(request).length > 0);
      return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
    },
    privacyVerifyProofV1(request) {
      assert.ok(Buffer.from(request).length > 0);
      return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
    },
    ...overrides,
  };
}

function captureThrown(fn) {
  try {
    fn();
  } catch (error) {
    return error;
  }
  assert.fail("expected function to throw");
}

test("package dist exposes the current general-purpose SDK entrypoint", () => {
  for (const name of [
    "AccountAddress",
    "ToriiClient",
    "ToriiBrowserClient",
    "buildTransaction",
    "noritoEncodeInstruction",
    "privacyCapabilitiesV1",
  ]) {
    assert.notEqual(packageExports[name], undefined, `${name} is exported`);
  }
});

test("package dist quantity builders reject numbers and noncanonical strings", () => {
  assert.equal(typeof packageExports.NumericV1?.decodeQuantityJson, "function");
  assert.equal(typeof packageExports.KotodamaQuantity, "function");
  const account = packageExports.AccountAddress.fromAccount({
    publicKey: Buffer.from(
      "B935AAF1F4E44B3DB79E5E5A9BA4569E6F3E2310C219F3DDD56D3277828D5480",
      "hex",
    ),
  }).toI105();
  const assetId = `62Fk4FPcMuLvW5QjDGNF2a4jAmjM#${account}`;
  for (const quantity of [1, -1, "+1", "01", "1.0", "1.2300", " 1", "1e0"]) {
    assert.throws(
      () => packageExports.buildMintAssetInstruction({ assetId, quantity }),
      /canonical|JavaScript numbers/u,
    );
  }
  assert.equal(
    packageExports.buildMintAssetInstruction({ assetId, quantity: 1n }).Mint.Asset.object,
    "1",
  );
});

test("package publishes the exact general-purpose subpath inventory", () => {
  assert.deepEqual(Object.keys(packageJson.exports).sort(), [
    ".",
    "./address",
    "./blake2b",
    "./browser",
    "./canonical-request",
    "./connect-browser",
    "./crypto",
    "./instruction-builders",
    "./ivm-artifact",
    "./kotodama-compiler",
    "./nexus-app",
    "./norito",
    "./normalizers",
    "./sccp",
    "./sorafs",
    "./torii",
    "./torii-browser",
    "./transaction-codec",
  ]);
});

test("package dist entrypoint exports privacy native archive helpers", () => {
  for (const name of [
    "isPrivacyNativeAvailable",
    "privacyCapabilitiesV1",
    "privacyProofRequestV1",
    "privacyBuildProofV1",
    "privacyVerifyProofV1",
  ]) {
    assert.equal(typeof packageExports[name], "function", `${name} must be exported`);
  }
  assert.equal(
    packageExports.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    64 * 1024 * 1024,
  );
  assert.equal(Number.isInteger(PRIVACY_REQUIRED_BRIDGE_ABI_VERSION), true);
});

test("package dist privacy native availability clears request copies after failures", () => {
  let throwingProofRequestProbe;
  let badProofRequestOutput;
  let throwingProbe;
  let badOutputProbe;
  let badOutput;

  withNativeBinding(
    completePrivacyBinding({
      privacyProofRequestV1(_algorithmId, _entrypoint, _vkRef, publicInputs) {
        throwingProofRequestProbe = publicInputs;
        throw new Error("proof-request probe failure after request copy");
      },
    }),
    () => assert.equal(isPrivacyNativeAvailable(), false),
  );
  withNativeBinding(
    completePrivacyBinding({
      privacyProofRequestV1() {
        badProofRequestOutput = Buffer.from([0x52]);
        return badProofRequestOutput;
      },
    }),
    () => assert.equal(isPrivacyNativeAvailable(), false),
  );
  withNativeBinding(
    completePrivacyBinding({
      privacyBuildProofV1(request) {
        throwingProbe = request;
        throw new Error("probe failure after request copy");
      },
    }),
    () => assert.equal(isPrivacyNativeAvailable(), false),
  );
  withNativeBinding(
    completePrivacyBinding({
      privacyVerifyProofV1(request) {
        badOutputProbe = request;
        badOutput = Buffer.from([0x56]);
        return badOutput;
      },
    }),
    () => assert.equal(isPrivacyNativeAvailable(), false),
  );

  assert.equal(throwingProofRequestProbe.every((value) => value === 0), true);
  assert.deepEqual(badProofRequestOutput, Buffer.alloc(1));
  assert.deepEqual(
    Buffer.from(throwingProbe),
    Buffer.alloc(privacyNoritoFrame(0x52).length),
  );
  assert.deepEqual(
    Buffer.from(badOutputProbe),
    Buffer.alloc(privacyNoritoFrame(0x52).length),
  );
  assert.deepEqual(badOutput, Buffer.alloc(1));
});

test("package dist privacy native wrappers reject invalid request archives", () => {
  withNativeBinding(
    completePrivacyBinding({
      privacyBuildProofV1() {
        assert.fail("invalid build request must not reach native dispatch");
      },
      privacyVerifyProofV1() {
        assert.fail("invalid verify request must not reach native dispatch");
      },
    }),
    () => {
      for (const wrongSchemaArchive of wrongSchemaPrivacyRequestArchives()) {
        assert.throws(
          () => privacyBuildProofV1(Buffer.from(wrongSchemaArchive)),
          /requestArchive must use the privacy request schema/,
        );
        assert.throws(
          () => privacyVerifyProofV1(Uint8Array.from(wrongSchemaArchive)),
          /requestArchive must use the privacy request schema/,
        );
      }
      for (const malformedArchive of malformedPrivacyRequestArchives()) {
        assert.throws(
          () => privacyBuildProofV1(Buffer.from(malformedArchive)),
          /requestArchive must be a valid Norito V1 archive/,
        );
        assert.throws(
          () => privacyVerifyProofV1(Uint8Array.from(malformedArchive)),
          /requestArchive must be a valid Norito V1 archive/,
        );
      }
    },
  );
});

test("package dist privacy native wrappers sanitize native exceptions", () => {
  const witness = Buffer.from("package-dist-private-witness-never-echo", "utf8");
  const requestArchive = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  const capturedRequests = [];
  const throwLeakingNativeError = (request) => {
    if (request !== undefined) {
      capturedRequests.push(request);
      assert.notEqual(request, requestArchive);
      assert.deepEqual(Buffer.from(request), PRIVACY_REQUEST_ARCHIVE);
    }
    throw new Error(`native panic included ${witness.toString("utf8")}`);
  };

  withNativeBinding(
    completePrivacyBinding({
      privacyCapabilitiesV1: throwLeakingNativeError,
      privacyBuildProofV1: throwLeakingNativeError,
      privacyVerifyProofV1: throwLeakingNativeError,
    }),
    () => {
      for (const [operation, invoke] of [
        ["privacyCapabilitiesV1", () => privacyCapabilitiesV1()],
        ["privacyBuildProofV1", () => privacyBuildProofV1(requestArchive)],
        ["privacyVerifyProofV1", () => privacyVerifyProofV1(requestArchive)],
      ]) {
        const error = captureThrown(invoke);
        assert.match(error.message, new RegExp(`native ${operation} failed`));
        assert.equal(error.cause, undefined);
        assert.equal(String(error).includes(witness.toString("utf8")), false);
        assert.equal(String(error.stack).includes(witness.toString("utf8")), false);
      }
    },
  );

  assert.equal(capturedRequests.length, 2);
  for (const request of capturedRequests) {
    assert.equal(request.every((value) => value === 0), true);
  }
  assert.deepEqual(requestArchive, PRIVACY_REQUEST_ARCHIVE);
});

test("package dist privacy native wrappers respect sliced request archive views", () => {
  const buildView = slicedPrivacyView(PRIVACY_REQUEST_ARCHIVE);
  const verifyBacking = Uint8Array.from([
    0x99,
    0x88,
    ...PRIVACY_REQUEST_ARCHIVE,
    0x77,
  ]);
  const verifyView = new DataView(
    verifyBacking.buffer,
    2,
    PRIVACY_REQUEST_ARCHIVE.length,
  );
  let buildRequest;
  let verifyRequest;

  withNativeBinding(
    completePrivacyBinding({
      privacyBuildProofV1(request) {
        buildRequest = request;
        assert.deepEqual(Buffer.from(request), PRIVACY_REQUEST_ARCHIVE);
        return slicedPrivacyView(PRIVACY_BUILD_ARCHIVE);
      },
      privacyVerifyProofV1(request) {
        verifyRequest = request;
        assert.deepEqual(Buffer.from(request), PRIVACY_REQUEST_ARCHIVE);
        const output = slicedPrivacyView(PRIVACY_VERIFY_ARCHIVE);
        return new DataView(output.buffer, output.byteOffset, output.byteLength);
      },
    }),
    () => {
      assert.deepEqual(privacyBuildProofV1(buildView), PRIVACY_BUILD_ARCHIVE);
      assert.deepEqual(privacyVerifyProofV1(verifyView), PRIVACY_VERIFY_ARCHIVE);
    },
  );

  assert.deepEqual(Buffer.from(buildView), PRIVACY_REQUEST_ARCHIVE);
  assert.deepEqual(
    Buffer.from(verifyBacking.subarray(2, 2 + PRIVACY_REQUEST_ARCHIVE.length)),
    PRIVACY_REQUEST_ARCHIVE,
  );
  assert.equal(buildRequest.every((value) => value === 0), true);
  assert.equal(verifyRequest.every((value) => value === 0), true);
});

test("package dist privacy native wrappers respect sliced native output archive views", () => {
  const prefixLength = 3;
  const capabilitiesBacking = Uint8Array.from([
    0xff,
    0x7f,
    0x50,
    ...PRIVACY_CAPABILITIES_ARCHIVE,
    0x24,
  ]);
  const buildBacking = Uint8Array.from([
    0xff,
    0x7f,
    0x42,
    ...PRIVACY_BUILD_ARCHIVE,
    0x13,
  ]);
  const verifyBacking = Uint8Array.from([
    0xff,
    0x7f,
    0x56,
    ...PRIVACY_VERIFY_ARCHIVE,
    0x37,
  ]);

  withNativeBinding(
    completePrivacyBinding({
      privacyCapabilitiesV1() {
        return capabilitiesBacking.subarray(
          prefixLength,
          prefixLength + PRIVACY_CAPABILITIES_ARCHIVE.length,
        );
      },
      privacyBuildProofV1() {
        return new DataView(
          buildBacking.buffer,
          prefixLength,
          PRIVACY_BUILD_ARCHIVE.length,
        );
      },
      privacyVerifyProofV1() {
        return verifyBacking.subarray(
          prefixLength,
          prefixLength + PRIVACY_VERIFY_ARCHIVE.length,
        );
      },
    }),
    () => {
      const capabilitiesArchive = privacyCapabilitiesV1();
      const buildArchive = privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE);
      const verifyArchive = privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE);

      assert.deepEqual(capabilitiesArchive, PRIVACY_CAPABILITIES_ARCHIVE);
      assert.deepEqual(buildArchive, PRIVACY_BUILD_ARCHIVE);
      assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);

      capabilitiesBacking[prefixLength] = 0x00;
      buildBacking[prefixLength] = 0x00;
      verifyBacking[prefixLength] = 0x00;

      assert.deepEqual(capabilitiesArchive, PRIVACY_CAPABILITIES_ARCHIVE);
      assert.deepEqual(buildArchive, PRIVACY_BUILD_ARCHIVE);
      assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);
    },
  );
});

test("package dist privacy native wrappers accept max-padded request archives", () => {
  withNativeBinding(completePrivacyBinding(), () => {
    assert.deepEqual(
      privacyBuildProofV1(privacyNoritoFrameWithPadding(0x52, 64)),
      PRIVACY_BUILD_ARCHIVE,
    );
    assert.deepEqual(
      privacyVerifyProofV1(privacyNoritoFrameWithPadding(0x52, 64)),
      PRIVACY_VERIFY_ARCHIVE,
    );
  });
});

test("package dist privacy native wrappers accept complete field-bitset flags", () => {
  const requestArchive = privacyNoritoFrameWithFlags(0x52, 0x26);
  const buildArchive = privacyNoritoFrameWithFlags(0x42, 0x26);
  const verifyArchive = privacyNoritoFrameWithFlags(0x56, 0x26);

  withNativeBinding(
    completePrivacyBinding({
      privacyBuildProofV1(request) {
        assert.deepEqual(Buffer.from(request), requestArchive);
        return buildArchive;
      },
      privacyVerifyProofV1(request) {
        assert.deepEqual(Buffer.from(request), requestArchive);
        return verifyArchive;
      },
    }),
    () => {
      assert.deepEqual(privacyBuildProofV1(requestArchive), buildArchive);
      assert.deepEqual(privacyVerifyProofV1(requestArchive), verifyArchive);
    },
  );
});

test("package dist privacy native wrappers defensively copy native output archives", () => {
  const capabilitiesOutput = Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
  const buildOutput = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  const verifyBacking = Uint8Array.from(
    Buffer.concat([Buffer.from([0x00]), PRIVACY_VERIFY_ARCHIVE, Buffer.from([0x00])]),
  );
  const verifyOutput = verifyBacking.subarray(1, 1 + PRIVACY_VERIFY_ARCHIVE.length);

  withNativeBinding(
    completePrivacyBinding({
      privacyCapabilitiesV1() {
        return capabilitiesOutput;
      },
      privacyBuildProofV1() {
        return buildOutput;
      },
      privacyVerifyProofV1() {
        return verifyOutput;
      },
    }),
    () => {
      const capabilitiesArchive = privacyCapabilitiesV1();
      assert.notEqual(capabilitiesArchive, capabilitiesOutput);
      assert.deepEqual(capabilitiesArchive, PRIVACY_CAPABILITIES_ARCHIVE);
      capabilitiesArchive[0] = 0x7f;
      assert.deepEqual(capabilitiesOutput, PRIVACY_CAPABILITIES_ARCHIVE);

      const buildArchive = privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE);
      assert.notEqual(buildArchive, buildOutput);
      assert.deepEqual(buildArchive, PRIVACY_BUILD_ARCHIVE);
      buildArchive[0] = 0x7f;
      assert.deepEqual(buildOutput, PRIVACY_BUILD_ARCHIVE);

      const verifyArchive = privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE);
      assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);
      verifyBacking[1] = 0x7f;
      assert.deepEqual(verifyArchive, PRIVACY_VERIFY_ARCHIVE);
    },
  );
});

test("package declarations mark privacy capability metadata readonly", () => {
  const declarations = readFileSync(new URL("../index.d.ts", import.meta.url), "utf8");
  assert.match(
    declarations,
    /export interface PrivacyCapabilities[\s\S]*readonly privacyAlgorithms:[\s\S]*readonly privacyCriteria:/,
  );
  assert.match(
    declarations,
    /export interface PrivacyProductionGate[\s\S]*readonly ready:[\s\S]*readonly missing:/,
  );
});

test("package dist privacy native wrappers reject invalid Norito-framed output archives", () => {
  const badMagic = Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
  badMagic[0] = 0x00;
  const badVersion = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badVersion[4] = 1;
  const badMinorVersion = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badMinorVersion[5] = 1;
  const badCompression = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badCompression[22] = 1;
  const badDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(0x42, 6n);
  const badOversizedDeclaredPayloadLength = privacyNoritoFrameWithDeclaredPayloadLength(
    0x42,
    0x8000000000000000n,
  );
  const badPadding = Buffer.concat([PRIVACY_VERIFY_ARCHIVE, Buffer.from([0x7f])]);
  const badExcessivePadding = privacyNoritoFrameWithPadding(0x42, 65);
  const badFlags = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badFlags[39] = 0x08;
  const badFieldBitsetFlags = Buffer.from(PRIVACY_BUILD_ARCHIVE);
  badFieldBitsetFlags[39] = 0x20;
  const badChecksum = Buffer.concat([PRIVACY_VERIFY_ARCHIVE, Buffer.alloc(1)]);
  badChecksum[31] = 0x01;
  const badPayload = Buffer.from(privacyNoritoFrameWithPayload(0x57));
  badPayload[44] ^= 0x7f;

  const cases = [
    ["privacyCapabilitiesV1", { privacyCapabilitiesV1: () => badMagic }, () => privacyCapabilitiesV1()],
    ["privacyBuildProofV1", { privacyBuildProofV1: () => badVersion }, () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE)],
    ["privacyBuildProofV1", { privacyBuildProofV1: () => badMinorVersion }, () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE)],
    ["privacyBuildProofV1", { privacyBuildProofV1: () => badCompression }, () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE)],
    ["privacyBuildProofV1", { privacyBuildProofV1: () => badDeclaredPayloadLength }, () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE)],
    ["privacyBuildProofV1", { privacyBuildProofV1: () => badOversizedDeclaredPayloadLength }, () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE)],
    ["privacyBuildProofV1", { privacyBuildProofV1: () => badFlags }, () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE)],
    ["privacyBuildProofV1", { privacyBuildProofV1: () => badFieldBitsetFlags }, () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE)],
    ["privacyBuildProofV1", { privacyBuildProofV1: () => badExcessivePadding }, () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE)],
    ["privacyVerifyProofV1", { privacyVerifyProofV1: () => badPadding }, () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE)],
    ["privacyVerifyProofV1", { privacyVerifyProofV1: () => badChecksum }, () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE)],
    ["privacyCapabilitiesV1", { privacyCapabilitiesV1: () => badPayload }, () => privacyCapabilitiesV1()],
  ];
  for (const [operation, override, invoke] of cases) {
    withNativeBinding(completePrivacyBinding(override), () => {
      assert.throws(
        invoke,
        new RegExp(`native ${operation} returned invalid Norito V1 archive`),
      );
    });
  }
});

test("package dist privacy native wrappers reject oversized request archives", () => {
  const oversized = Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
  withNativeBinding(
    completePrivacyBinding({
      privacyBuildProofV1() {
        assert.fail("oversized build request must not reach native dispatch");
      },
      privacyVerifyProofV1() {
        assert.fail("oversized verify request must not reach native dispatch");
      },
    }),
    () => {
      assert.throws(() => privacyBuildProofV1(oversized), /must not exceed/);
      assert.throws(() => privacyVerifyProofV1(oversized), /must not exceed/);
    },
  );
});

test("package dist privacy native availability probes reject unsafe raw output", () => {
  const overrides = [
    { privacyCapabilitiesV1: () => "json is not Norito" },
    { privacyProofRequestV1: () => "json is not Norito" },
    { privacyProofRequestV1: () => Buffer.from(PRIVACY_BUILD_ARCHIVE) },
    { privacyBuildProofV1: () => new Uint8Array() },
    { privacyVerifyProofV1: () => undefined },
    { privacyBuildProofV1: () => [0x42] },
    {
      privacyCapabilitiesV1: () =>
        Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f),
    },
    {
      privacyProofRequestV1: () =>
        Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f),
    },
    {
      privacyBuildProofV1: () =>
        Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f),
    },
    {
      privacyVerifyProofV1: () =>
        Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f),
    },
  ];
  for (const archive of malformedPrivacyNativeOutputArchives(0x50)) {
    overrides.push({ privacyCapabilitiesV1: () => Buffer.from(archive) });
  }
  for (const archive of malformedPrivacyNativeOutputArchives(0x52)) {
    overrides.push({ privacyProofRequestV1: () => Buffer.from(archive) });
  }
  for (const archive of malformedPrivacyNativeOutputArchives(0x42)) {
    overrides.push({ privacyBuildProofV1: () => Buffer.from(archive) });
  }
  for (const archive of malformedPrivacyNativeOutputArchives(0x56)) {
    overrides.push({ privacyVerifyProofV1: () => Buffer.from(archive) });
  }
  for (const override of overrides) {
    withNativeBinding(completePrivacyBinding(override), () => {
      assert.equal(isPrivacyNativeAvailable(), false);
    });
  }
});

test("package dist privacy native wrappers reject wrong-operation result schemas", () => {
  for (const [operation, override, invoke] of [
    [
      "privacyCapabilitiesV1",
      {
        privacyCapabilitiesV1: () =>
          privacyNoritoFrameWithSchemaOverride(0x50, 21, 0x42),
      },
      () => privacyCapabilitiesV1(),
    ],
    [
      "privacyBuildProofV1",
      {
        privacyBuildProofV1: () =>
          privacyNoritoFrameWithSchemaOverride(0x42, 6, 0x56),
      },
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
    ],
    [
      "privacyVerifyProofV1",
      {
        privacyVerifyProofV1: () =>
          privacyNoritoFrameWithSchemaOverride(0x56, 21, 0x50),
      },
      () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
    ],
  ]) {
    withNativeBinding(completePrivacyBinding(override), () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        invoke,
        new RegExp(`native ${operation} returned unexpected privacy result schema`),
      );
    });
  }
});
