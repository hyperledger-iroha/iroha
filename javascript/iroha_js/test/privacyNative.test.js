import assert from "node:assert/strict";
import test from "node:test";

import {
  PRIVACY_FFI_ERROR_INVALID_REQUEST,
  PRIVACY_FFI_ERROR_MALFORMED_NORITO,
  PRIVACY_FFI_ERROR_NULL_POINTER,
  PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
  PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
  PRIVACY_FFI_STATUS_ERROR,
  PRIVACY_FFI_VERSION_V1,
  PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
  PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
  buildZkAceTransferAuthorizationV1,
  isPrivacyNativeAvailable,
  privacyBuildProofV1,
  privacyCapabilitiesV1,
  privacyVerifyProofV1,
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

function slicedPrivacyView(archive, prefix = [0xff, 0x7f, 0x42], suffix = [0x24, 0x13]) {
  const backing = Uint8Array.from([
    ...prefix,
    ...archive,
    ...suffix,
  ]);
  return backing.subarray(prefix.length, prefix.length + archive.length);
}

function int8PrivacyFrame(schemaByte) {
  const frame = privacyNoritoFrameWithPayload(schemaByte);
  const backing = new ArrayBuffer(frame.length);
  new Uint8Array(backing).set(frame);
  return new Int8Array(backing);
}

function sharedPrivacyFrame(schemaByte) {
  const frame = privacyNoritoFrameWithPayload(schemaByte);
  const backing = new SharedArrayBuffer(frame.length);
  new Uint8Array(backing).set(frame);
  return new Uint8Array(backing);
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

test("privacy native availability requires all raw archive methods", () => {
  withNativeBinding({}, () => {
    assert.equal(isPrivacyNativeAvailable(), false);
  });
  withNativeBinding({ privacyCapabilitiesV1() {} }, () => {
    assert.equal(isPrivacyNativeAvailable(), false);
  });
  withNativeBinding(completePrivacyBinding({ connectNoritoBridgeAbiVersion: undefined }), () => {
    assert.equal(isPrivacyNativeAvailable(), false);
  });
  withNativeBinding(
    completePrivacyBinding({
      connectNoritoBridgeAbiVersion() {
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION - 1;
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
    },
  );
  withNativeBinding(
    completePrivacyBinding({
      connectNoritoBridgeAbiVersion() {
        return "not an ABI version";
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
    },
  );
  for (const abiVersion of [
    "6",
    true,
    -1,
    6.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    Number.MAX_SAFE_INTEGER + 1,
    0x1_0000_0000,
  ]) {
    withNativeBinding(
      completePrivacyBinding({
        connectNoritoBridgeAbiVersion() {
          return abiVersion;
        },
      }),
      () => {
        assert.equal(isPrivacyNativeAvailable(), false);
      },
    );
  }
  withNativeBinding(
    completePrivacyBinding({
      connectNoritoBridgeAbiVersion() {
        throw new Error("stale native ABI probe");
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        () => privacyCapabilitiesV1(),
        /privacyCapabilitiesV1 requires the iroha_js_host native binding built with privacy FFI support/,
      );
    },
  );
  withNativeBinding(completePrivacyBinding(), () => {
    assert.equal(isPrivacyNativeAvailable(), true);
  });
});

test("privacy FFI deterministic error constants are public", () => {
  assert.equal(PRIVACY_FFI_VERSION_V1, 1);
  assert.equal(PRIVACY_REQUIRED_BRIDGE_ABI_VERSION, 6);
  assert.equal(PRIVACY_FFI_STATUS_ERROR, 1);
  assert.equal(PRIVACY_FFI_ERROR_NULL_POINTER, 1);
  assert.equal(PRIVACY_FFI_ERROR_MALFORMED_NORITO, 2);
  assert.equal(PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM, 3);
  assert.equal(PRIVACY_FFI_ERROR_PRODUCTION_DISABLED, 4);
  assert.equal(PRIVACY_FFI_ERROR_INVALID_REQUEST, 5);
});

function validZkAceAuthorizationPayload() {
  return JSON.stringify({
    public_inputs: { ok: true },
    proof: {
      backend: "stark/fri",
      proof_b64: "AA==",
      vk_ref: {
        backend: "stark/fri",
        name: "zk_ace_pq_authorization_v0",
      },
    },
    identity_commitment: "11",
    tx_digest: "22",
    replay_nullifier: "33",
    policy_hash: "44",
    verifier_key_id: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
  });
}

function validZkAceTransferAuthorizationOptions(overrides = {}) {
  return {
    fromAccountId: "alice@wonderland",
    toAccountId: "bob@wonderland",
    assetDefinitionId: "xor#wonderland",
    amount: "17",
    chainId: "wonderland",
    identityRoot: Buffer.alloc(32, 0x31),
    identityBlinding: Buffer.alloc(32, 0x32),
    replaySecret: Buffer.alloc(32, 0x33),
    policyHash: Buffer.alloc(32, 0x34),
    verifierKeyId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    verifyingKeyCommitment: Buffer.alloc(32, 0x55),
    ...overrides,
  };
}

test("ZK-ACE transfer authorization rejects malformed amounts before native dispatch", () => {
  let nativeCalls = 0;
  let stringified = false;
  const hostileAmount = {
    toString() {
      stringified = true;
      return "17";
    },
  };
  const invalidAmounts = [
    undefined,
    null,
    "",
    " ",
    "0",
    "0000",
    "-1",
    "+1",
    "1.0",
    "1e3",
    0,
    -1,
    1.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    0n,
    -1n,
    1n << 128n,
    true,
    [],
    hostileAmount,
  ];

  withNativeBinding(
    {
      zkAceBuildTransferAuthorizationV1() {
        nativeCalls += 1;
        return validZkAceAuthorizationPayload();
      },
    },
    () => {
      for (const amount of invalidAmounts) {
        const error = captureThrown(() =>
          buildZkAceTransferAuthorizationV1(
            validZkAceTransferAuthorizationOptions({ amount }),
          ),
        );
        assert.match(error.message, /amount must be a positive decimal u128 string/);
      }
    },
  );

  assert.equal(nativeCalls, 0);
  assert.equal(stringified, false);
});

test("ZK-ACE transfer authorization canonicalizes positive u128 amounts before native dispatch", () => {
  const capturedAmounts = [];
  const u128Max = (1n << 128n) - 1n;

  withNativeBinding(
    {
      zkAceBuildTransferAuthorizationV1(_from, _to, _asset, amount) {
        capturedAmounts.push(amount);
        return validZkAceAuthorizationPayload();
      },
    },
    () => {
      buildZkAceTransferAuthorizationV1(
        validZkAceTransferAuthorizationOptions({ amount: "00017" }),
      );
      buildZkAceTransferAuthorizationV1(
        validZkAceTransferAuthorizationOptions({ amount: 23 }),
      );
      buildZkAceTransferAuthorizationV1(
        validZkAceTransferAuthorizationOptions({ amount: u128Max }),
      );
    },
  );

  assert.deepEqual(capturedAmounts, ["17", "23", u128Max.toString(10)]);
});

test("ZK-ACE transfer authorization sanitizes production-disabled native errors", () => {
  const secret = Buffer.from("js-zk-ace-private-secret-1234567", "utf8");
  const proof = "candidate-zk-ace-proof";
  const capturedCalls = [];

  withNativeBinding(
    {
      zkAceBuildTransferAuthorizationV1(...args) {
        capturedCalls.push(args);
        throw new Error(
          `PRIVACY_FFI_ERROR_PRODUCTION_DISABLED zk-ace-pq-authorization-v0 ` +
            `buildZkAceAuthorizationProofV1 ` +
            `stark-fri:zk_ace_pq_authorization_v0 ` +
            `Iroha production allowlist ${secret.toString("utf8")} ${proof}`,
        );
      },
    },
    () => {
      const error = captureThrown(() =>
        buildZkAceTransferAuthorizationV1(
          validZkAceTransferAuthorizationOptions({ replaySecret: secret }),
        ),
      );

      assert.match(error.message, /PRIVACY_FFI_ERROR_PRODUCTION_DISABLED/);
      assert.match(error.message, /zk-ace-pq-authorization-v0/);
      assert.match(error.message, /buildZkAceAuthorizationProofV1/);
      assert.match(error.message, /stark-fri:zk_ace_pq_authorization_v0/);
      assert.match(error.message, /Iroha production allowlist/);
      assert.equal(error.message.includes(secret.toString("utf8")), false);
      assert.equal(error.message.includes(proof), false);
      assert.equal(String(error.stack).includes(secret.toString("utf8")), false);
      assert.equal(String(error.stack).includes(proof), false);
      assert.equal(error.cause, undefined);
    },
  );

  assert.equal(capturedCalls.length, 1);
  assert.equal(capturedCalls[0][0], "alice@wonderland");
  assert.equal(capturedCalls[0][5].length, 32);
  assert.deepEqual(capturedCalls[0][7], secret);
  assert.equal(
    capturedCalls[0][9],
    "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
  );
  assert.deepEqual(capturedCalls[0][10], Buffer.alloc(32, 0x55));
});

test("privacy native availability probes build and verify with Norito request archives", () => {
  const legacyTextProbe = Buffer.from(
    "iroha-privacy-native-availability-probe-v1",
    "utf8",
  );
  const expectedProbe = privacyNoritoFrame(0x52);
  let buildProbe;
  let verifyProbe;
  let buildProbeAfterReturn;
  let verifyProbeAfterReturn;
  let capabilitiesOutput;
  let buildOutput;
  let verifyOutput;

  withNativeBinding(
    completePrivacyBinding({
      privacyCapabilitiesV1() {
        capabilitiesOutput = Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
        return capabilitiesOutput;
      },
      privacyBuildProofV1(request) {
        buildProbe = Buffer.from(request);
        buildProbeAfterReturn = request;
        buildOutput = Buffer.from(PRIVACY_BUILD_ARCHIVE);
        return buildOutput;
      },
      privacyVerifyProofV1(request) {
        verifyProbe = Buffer.from(request);
        verifyProbeAfterReturn = request;
        verifyOutput = Buffer.from(PRIVACY_VERIFY_ARCHIVE);
        return verifyOutput;
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), true);
    },
  );

  assert.deepEqual(buildProbe, expectedProbe);
  assert.deepEqual(verifyProbe, expectedProbe);
  assert.notDeepEqual(buildProbe, legacyTextProbe);
  assert.notDeepEqual(verifyProbe, legacyTextProbe);
  assert.equal(buildProbeAfterReturn.every((value) => value === 0), true);
  assert.equal(verifyProbeAfterReturn.every((value) => value === 0), true);
  assert.equal(capabilitiesOutput.every((value) => value === 0), true);
  assert.equal(buildOutput.every((value) => value === 0), true);
  assert.equal(verifyOutput.every((value) => value === 0), true);
});

test("privacy native availability probes clear request copies after native failures", () => {
  let throwingProbe;
  let badOutputProbe;
  let badOutput;

  withNativeBinding(
    completePrivacyBinding({
      privacyBuildProofV1(request) {
        throwingProbe = request;
        throw new Error("probe failure after request copy");
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
    },
  );

  withNativeBinding(
    completePrivacyBinding({
      privacyVerifyProofV1(request) {
        badOutputProbe = request;
        badOutput = Buffer.from([0x56]);
        return badOutput;
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
    },
  );

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

test("privacy native availability probes reject unsafe raw output", () => {
  const overrides = [
    {
      privacyCapabilitiesV1() {
        return "json is not Norito";
      },
    },
    {
      privacyBuildProofV1() {
        return new Uint8Array();
      },
    },
    {
      privacyVerifyProofV1() {
        return undefined;
      },
    },
    {
      privacyBuildProofV1() {
        return [0x42];
      },
    },
    {
      privacyBuildProofV1() {
        return Buffer.from([0x42]);
      },
    },
    {
      privacyCapabilitiesV1() {
        const bad = Buffer.from(PRIVACY_CAPABILITIES_ARCHIVE);
        bad[0] = 0x00;
        return bad;
      },
    },
    {
      privacyBuildProofV1() {
        const bad = Buffer.from(PRIVACY_BUILD_ARCHIVE);
        bad[39] = 0x08;
        return bad;
      },
    },
    {
      privacyVerifyProofV1() {
        return Buffer.concat([PRIVACY_VERIFY_ARCHIVE, Buffer.from([0x01])]);
      },
    },
    {
      privacyVerifyProofV1() {
        const bad = Buffer.concat([PRIVACY_VERIFY_ARCHIVE, Buffer.alloc(1)]);
        bad[31] = 0x01;
        return bad;
      },
    },
    {
      privacyVerifyProofV1() {
        throw new Error("native probe failed with witness-like bytes");
      },
    },
    {
      privacyCapabilitiesV1() {
        return Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
      },
    },
    {
      privacyBuildProofV1() {
        return Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
      },
    },
    {
      privacyVerifyProofV1() {
        return Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
      },
    },
  ];

  for (const archive of malformedPrivacyNativeOutputArchives(0x50)) {
    overrides.push({
      privacyCapabilitiesV1() {
        return Buffer.from(archive);
      },
    });
  }
  for (const archive of malformedPrivacyNativeOutputArchives(0x42)) {
    overrides.push({
      privacyBuildProofV1() {
        return Buffer.from(archive);
      },
    });
  }
  for (const archive of malformedPrivacyNativeOutputArchives(0x56)) {
    overrides.push({
      privacyVerifyProofV1() {
        return Buffer.from(archive);
      },
    });
  }

  for (const override of overrides) {
    withNativeBinding(completePrivacyBinding(override), () => {
      assert.equal(isPrivacyNativeAvailable(), false);
    });
  }
});

test("privacy native wrappers return opaque archive bytes", () => {
  withNativeBinding(completePrivacyBinding(), () => {
    assert.deepEqual(privacyCapabilitiesV1(), PRIVACY_CAPABILITIES_ARCHIVE);
    assert.deepEqual(privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE), PRIVACY_BUILD_ARCHIVE);
    assert.deepEqual(privacyVerifyProofV1(Uint8Array.from(PRIVACY_REQUEST_ARCHIVE)), PRIVACY_VERIFY_ARCHIVE);
  });
});

test("privacy native wrappers respect sliced request archive views", () => {
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
        return new DataView(
          slicedPrivacyView(PRIVACY_VERIFY_ARCHIVE).buffer,
          3,
          PRIVACY_VERIFY_ARCHIVE.length,
        );
      },
    }),
    () => {
      assert.deepEqual(privacyBuildProofV1(buildView), PRIVACY_BUILD_ARCHIVE);
      assert.deepEqual(privacyVerifyProofV1(verifyView), PRIVACY_VERIFY_ARCHIVE);
    },
  );

  assert.deepEqual(Buffer.from(buildView), PRIVACY_REQUEST_ARCHIVE);
  assert.deepEqual(Buffer.from(verifyBacking.subarray(2, 2 + PRIVACY_REQUEST_ARCHIVE.length)), PRIVACY_REQUEST_ARCHIVE);
  assert.equal(buildRequest.every((value) => value === 0), true);
  assert.equal(verifyRequest.every((value) => value === 0), true);
});

test("privacy native wrappers respect sliced native output archive views", () => {
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

test("privacy native wrappers defensively copy native output archives", () => {
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

test("privacy native wrappers clear temporary request copies after native dispatch", () => {
  const requestArchive = Buffer.from(PRIVACY_REQUEST_ARCHIVE);
  const originalArchive = Buffer.from(requestArchive);
  let buildRequest;
  let verifyRequest;

  withNativeBinding(
    completePrivacyBinding({
      privacyBuildProofV1(request) {
        buildRequest = request;
        assert.notEqual(request, requestArchive);
        assert.deepEqual(Buffer.from(request), originalArchive);
        return Uint8Array.from(PRIVACY_BUILD_ARCHIVE);
      },
      privacyVerifyProofV1(request) {
        verifyRequest = request;
        assert.notEqual(request, requestArchive);
        assert.deepEqual(Buffer.from(request), originalArchive);
        return Uint8Array.from(PRIVACY_VERIFY_ARCHIVE);
      },
    }),
    () => {
      assert.deepEqual(privacyBuildProofV1(requestArchive), PRIVACY_BUILD_ARCHIVE);
      assert.deepEqual(privacyVerifyProofV1(requestArchive), PRIVACY_VERIFY_ARCHIVE);
    },
  );

  assert.ok(buildRequest, "build request should be captured");
  assert.ok(verifyRequest, "verify request should be captured");
  assert.equal(buildRequest.every((value) => value === 0), true);
  assert.equal(verifyRequest.every((value) => value === 0), true);
  assert.deepEqual(requestArchive, originalArchive);
});

test("privacy native wrappers reject empty request archives before native calls", () => {
  withNativeBinding(completePrivacyBinding(), () => {
    assert.throws(
      () => privacyBuildProofV1(Buffer.alloc(0)),
      /requestArchive must not be empty/,
    );
    assert.throws(
      () => privacyVerifyProofV1(new Uint8Array()),
      /requestArchive must not be empty/,
    );
  });
});

test("privacy native wrappers reject oversized request archives before native calls", () => {
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
      assert.throws(
        () => privacyBuildProofV1(oversized),
        /requestArchive must not exceed/,
      );
      assert.throws(
        () => privacyVerifyProofV1(oversized),
        /requestArchive must not exceed/,
      );
    },
  );
});

test("privacy native wrappers require binary Norito request archives", () => {
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
      assert.throws(
        () => privacyBuildProofV1("not norito"),
        /requestArchive must be Norito V1 bytes, not a string/,
      );
      assert.throws(
        () => privacyVerifyProofV1("not norito"),
        /requestArchive must be Norito V1 bytes, not a string/,
      );
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

test("privacy native wrappers accept max-padded Norito request archives", () => {
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

test("privacy native wrappers accept complete field-bitset Norito flags", () => {
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

test("privacy native wrappers fail when methods are missing", () => {
  withNativeBinding({ privacyCapabilitiesV1() {} }, () => {
    assert.throws(
      () => privacyCapabilitiesV1(),
      /privacyCapabilitiesV1 requires the iroha_js_host native binding built with privacy FFI support/,
    );
    assert.throws(
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
      /privacyBuildProofV1 requires the iroha_js_host native binding built with privacy FFI support/,
    );
  });
});

test("privacy native wrappers reject missing and empty native output", () => {
  withNativeBinding(
    completePrivacyBinding({
      privacyCapabilitiesV1() {
        return undefined;
      },
      privacyBuildProofV1() {
        return Uint8Array.from([]);
      },
      privacyVerifyProofV1() {
        return null;
      },
    }),
    () => {
      assert.throws(
        () => privacyCapabilitiesV1(),
        /native privacyCapabilitiesV1 returned no output/,
      );
      assert.throws(
        () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyBuildProofV1 returned empty output/,
      );
      assert.throws(
        () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyVerifyProofV1 returned no output/,
      );
    },
  );
});

test("privacy native wrappers reject oversized native output archives", () => {
  const oversized = Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f);
  withNativeBinding(
    completePrivacyBinding({
      privacyCapabilitiesV1() {
        return oversized;
      },
      privacyBuildProofV1() {
        return oversized;
      },
      privacyVerifyProofV1() {
        return oversized;
      },
    }),
    () => {
      assert.throws(
        () => privacyCapabilitiesV1(),
        /native privacyCapabilitiesV1 returned oversized output/,
      );
      assert.throws(
        () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyBuildProofV1 returned oversized output/,
      );
      assert.throws(
        () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyVerifyProofV1 returned oversized output/,
      );
    },
  );
});

test("privacy native wrappers reject wrong-operation result schemas", () => {
  for (const [operation, override, invoke] of [
    [
      "privacyCapabilitiesV1",
      { privacyCapabilitiesV1: () => privacyNoritoFrameWithSchemaOverride(0x50, 21, 0x42) },
      () => privacyCapabilitiesV1(),
    ],
    [
      "privacyBuildProofV1",
      { privacyBuildProofV1: () => privacyNoritoFrameWithSchemaOverride(0x42, 6, 0x56) },
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
    ],
    [
      "privacyVerifyProofV1",
      { privacyVerifyProofV1: () => privacyNoritoFrameWithSchemaOverride(0x56, 21, 0x50) },
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

test("privacy native wrappers reject invalid Norito-framed native output", () => {
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

  withNativeBinding(
    completePrivacyBinding({
      privacyCapabilitiesV1() {
        return badMagic;
      },
      privacyBuildProofV1() {
        return badVersion;
      },
      privacyVerifyProofV1() {
        return badPadding;
      },
    }),
    () => {
      assert.throws(
        () => privacyCapabilitiesV1(),
        /native privacyCapabilitiesV1 returned invalid Norito V1 archive/,
      );
      assert.throws(
        () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyBuildProofV1 returned invalid Norito V1 archive/,
      );
      assert.throws(
        () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyVerifyProofV1 returned invalid Norito V1 archive/,
      );
    },
  );

  for (const [operation, override, invoke] of [
    ["privacyCapabilitiesV1", { privacyCapabilitiesV1: () => badPayload }, () => privacyCapabilitiesV1()],
    [
      "privacyBuildProofV1",
      { privacyBuildProofV1: () => badMinorVersion },
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
    ],
    [
      "privacyBuildProofV1",
      { privacyBuildProofV1: () => badCompression },
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
    ],
    [
      "privacyBuildProofV1",
      { privacyBuildProofV1: () => badDeclaredPayloadLength },
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
    ],
    [
      "privacyBuildProofV1",
      { privacyBuildProofV1: () => badOversizedDeclaredPayloadLength },
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
    ],
    [
      "privacyBuildProofV1",
      { privacyBuildProofV1: () => badFlags },
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
    ],
    [
      "privacyBuildProofV1",
      { privacyBuildProofV1: () => badFieldBitsetFlags },
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
    ],
    [
      "privacyBuildProofV1",
      { privacyBuildProofV1: () => badExcessivePadding },
      () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
    ],
    [
      "privacyVerifyProofV1",
      { privacyVerifyProofV1: () => badChecksum },
      () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
    ],
  ]) {
    withNativeBinding(completePrivacyBinding(override), () => {
      assert.throws(invoke, new RegExp(`native ${operation} returned invalid Norito V1 archive`));
    });
  }
});

test("privacy native wrappers reject textual native output", () => {
  withNativeBinding(
    completePrivacyBinding({
      privacyCapabilitiesV1() {
        return "json is not a Norito archive";
      },
      privacyBuildProofV1() {
        return "not bytes";
      },
      privacyVerifyProofV1() {
        return "not bytes";
      },
    }),
    () => {
      assert.throws(
        () => privacyCapabilitiesV1(),
        /native privacyCapabilitiesV1 returned text instead of Norito V1 bytes/,
      );
      assert.throws(
        () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyBuildProofV1 returned text instead of Norito V1 bytes/,
      );
      assert.throws(
        () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyVerifyProofV1 returned text instead of Norito V1 bytes/,
      );
    },
  );
});

test("privacy native wrappers reject non-byte native output", () => {
  withNativeBinding(
    completePrivacyBinding({
      privacyCapabilitiesV1() {
        return [0x50, PRIVACY_FFI_VERSION_V1];
      },
      privacyBuildProofV1() {
        return { proof: [0x42] };
      },
      privacyVerifyProofV1() {
        return [0x56];
      },
    }),
    () => {
      assert.throws(
        () => privacyCapabilitiesV1(),
        /native privacyCapabilitiesV1 output must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer/,
      );
      assert.throws(
        () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyBuildProofV1 output must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer/,
      );
      assert.throws(
        () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyVerifyProofV1 output must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer/,
      );
    },
  );
});

test("privacy native wrappers reject ambiguous byte views", () => {
  withNativeBinding(
    completePrivacyBinding({
      privacyCapabilitiesV1() {
        return int8PrivacyFrame(0x50);
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        () => privacyCapabilitiesV1(),
        /native privacyCapabilitiesV1 output must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer/,
      );
    },
  );

  withNativeBinding(
    completePrivacyBinding({
      privacyBuildProofV1() {
        return new Uint16Array(24);
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        () => privacyBuildProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyBuildProofV1 output must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer/,
      );
    },
  );

  withNativeBinding(
    completePrivacyBinding({
      privacyBuildProofV1() {
        assert.fail("signed typed-array build request must not reach native dispatch");
      },
      privacyVerifyProofV1() {
        assert.fail("wide typed-array verify request must not reach native dispatch");
      },
    }),
    () => {
      assert.throws(
        () => privacyBuildProofV1(int8PrivacyFrame(0x52)),
        /requestArchive must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer/,
      );
      assert.throws(
        () => privacyVerifyProofV1(new Uint16Array(24)),
        /requestArchive must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer/,
      );
    },
  );

  withNativeBinding(
    completePrivacyBinding({
      privacyBuildProofV1() {
        assert.fail("shared-memory build request must not reach native dispatch");
      },
      privacyVerifyProofV1() {
        return sharedPrivacyFrame(0x56);
      },
    }),
    () => {
      assert.throws(
        () => privacyBuildProofV1(sharedPrivacyFrame(0x52)),
        /requestArchive must not use shared memory/,
      );
      assert.throws(
        () => privacyVerifyProofV1(PRIVACY_REQUEST_ARCHIVE),
        /native privacyVerifyProofV1 output must not use shared memory/,
      );
    },
  );
});

test("privacy native wrappers sanitize native exceptions before exposing request bytes", () => {
  const witness = Buffer.from("js-sdk-private-witness-never-echo-9b65", "utf8");
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
  assert.deepEqual(requestArchive, Buffer.from(PRIVACY_REQUEST_ARCHIVE));
});
