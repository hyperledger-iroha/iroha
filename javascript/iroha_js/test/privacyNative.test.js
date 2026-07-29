import assert from "node:assert/strict";
import test from "node:test";

import * as cryptoSurface from "../src/crypto.js";
import * as rootSurface from "../src/index.js";
import {
  PRIVACY_CAPABILITY_VALIDATION_STATUS_V1,
  PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
  PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
  isPrivacyNativeAvailable,
  privacyCapabilitiesV1,
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

function validCapabilityArchive() {
  const frame = Buffer.alloc(43);
  frame.write("NRT0", 0, "ascii");
  frame.fill(0x50, 6, 22);
  frame.writeBigUInt64LE(3n, 23);
  Buffer.from([0xb9, 0xd3, 0xa8, 0x0c, 0xcd, 0x5d, 0x13, 0x24]).copy(frame, 31);
  Buffer.from([0xa5, 0x5a, 0x11]).copy(frame, 40);
  return frame;
}

function crc64Xz(payload) {
  let crc = 0xffff_ffff_ffff_ffffn;
  for (const byte of payload) {
    crc ^= BigInt(byte);
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 1n) !== 0n
          ? (crc >> 1n) ^ 0xc96c_5795_d787_0f42n
          : crc >> 1n;
    }
  }
  return BigInt.asUintN(64, crc ^ 0xffff_ffff_ffff_ffffn);
}

function crcValidOneByteFake() {
  const frame = Buffer.alloc(41);
  frame.write("NRT0", 0, "ascii");
  frame.fill(0x50, 6, 22);
  frame.writeBigUInt64LE(1n, 23);
  frame[40] = 0xa5;
  frame.writeBigUInt64LE(crc64Xz(frame.subarray(40)), 31);
  return frame;
}

function capabilityBinding(overrides = {}) {
  const canonicalArchive = validCapabilityArchive();
  return {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCapabilitiesV1() {
      return Uint8Array.from(canonicalArchive);
    },
    privacyValidateCapabilitiesV1(archive) {
      return Buffer.from(archive).equals(canonicalArchive)
        ? PRIVACY_CAPABILITY_VALIDATION_STATUS_V1.VALID
        : PRIVACY_CAPABILITY_VALIDATION_STATUS_V1.MALFORMED_ARCHIVE;
    },
    ...overrides,
  };
}

test("privacy native surface contains only the capability snapshot bridge", () => {
  for (const surface of [cryptoSurface, rootSurface]) {
    assert.equal(typeof surface.privacyCapabilitiesV1, "function");
    for (const retired of [
      "privacyProofRequestV1",
      "privacyBuildProofV1",
      "privacyVerifyProofV1",
      "PRIVACY_FFI_STATUS_ERROR",
      "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
    ]) {
      assert.equal(retired in surface, false, retired);
    }
  }
});

test("privacy native availability requires exact ABI plus the shared typed validator", () => {
  withNativeBinding({}, () => assert.equal(isPrivacyNativeAvailable(), false));
  withNativeBinding(
    capabilityBinding({ connectNoritoBridgeAbiVersion: undefined }),
    () => assert.equal(isPrivacyNativeAvailable(), false),
  );
  withNativeBinding(
    capabilityBinding({ privacyValidateCapabilitiesV1: undefined }),
    () => assert.equal(isPrivacyNativeAvailable(), false),
  );
  withNativeBinding(
    capabilityBinding({
      connectNoritoBridgeAbiVersion() {
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION - 1;
      },
    }),
    () => assert.equal(isPrivacyNativeAvailable(), false),
  );
  withNativeBinding(
    capabilityBinding({
      connectNoritoBridgeAbiVersion() {
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION + 1;
      },
    }),
    () => assert.equal(isPrivacyNativeAvailable(), false),
  );
  withNativeBinding(capabilityBinding(), () => {
    assert.equal(isPrivacyNativeAvailable(), true);
  });
});

test("privacyCapabilitiesV1 returns a defensive copy of a valid Norito archive", () => {
  const nativeArchive = validCapabilityArchive();
  withNativeBinding(
    capabilityBinding({
      privacyCapabilitiesV1() {
        return nativeArchive;
      },
    }),
    () => {
      const returned = privacyCapabilitiesV1();
      assert.deepEqual(returned, validCapabilityArchive());
      returned.fill(0);
      assert.deepEqual(nativeArchive, validCapabilityArchive());
    },
  );
});

test("privacyCapabilitiesV1 rejects every output the shared typed validator rejects", () => {
  const malformed = [
    Buffer.alloc(0),
    Buffer.from([0x50]),
    Buffer.from(validCapabilityArchive()),
  ];
  malformed[2][0] ^= 0xff;

  for (const archive of malformed) {
    withNativeBinding(
      capabilityBinding({ privacyCapabilitiesV1: () => archive }),
      () => assert.throws(() => privacyCapabilitiesV1()),
    );
  }

  const wrongSchema = validCapabilityArchive();
  wrongSchema.fill(0x42, 6, 22);
  withNativeBinding(
    capabilityBinding({ privacyCapabilitiesV1: () => wrongSchema }),
    () => assert.throws(() => privacyCapabilitiesV1(), /invalid typed privacy capability/),
  );

  withNativeBinding(
    capabilityBinding({ privacyCapabilitiesV1: () => crcValidOneByteFake() }),
    () => assert.throws(() => privacyCapabilitiesV1(), /invalid typed privacy capability/),
  );

  withNativeBinding(
    capabilityBinding({
      privacyCapabilitiesV1: () => Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1),
    }),
    () => assert.throws(() => privacyCapabilitiesV1(), /oversized output/),
  );
});

test("privacyCapabilitiesV1 sanitizes native exceptions", () => {
  withNativeBinding(
    capabilityBinding({
      privacyCapabilitiesV1() {
        throw new Error("secret native detail");
      },
    }),
    () => {
      assert.throws(
        () => privacyCapabilitiesV1(),
        (error) =>
          error.message === "native privacyCapabilitiesV1 failed" &&
          !error.message.includes("secret native detail"),
      );
    },
  );
});
