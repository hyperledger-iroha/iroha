import assert from "node:assert/strict";
import test from "node:test";

import * as cryptoSurface from "../src/crypto.js";
import * as cryptoSubpathSurface from "../src/public/crypto.js";
import * as rootSurface from "../src/index.js";
import {
  PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES,
  PRIVACY_COMPILED_PROFILE_CATALOG_VALIDATION_STATUS_V1,
  PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
  _createCryptoApi,
} from "../src/crypto.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";

const RETIRED_STATIC_CRYPTO_CAPABILITY_LIST =
  ["SUPPORTED", "CRYPTO", "ALGORITHMS"].join("_");

function withCryptoApi(binding, fn) {
  return fn(_createCryptoApi(createNativeRuntime(binding)));
}

function validCompiledProfileCatalogArchive() {
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

function compiledProfileCatalogBinding(overrides = {}) {
  const canonicalArchive = validCompiledProfileCatalogArchive();
  return {
    connectNoritoBridgeAbiVersion() {
      return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION;
    },
    privacyCompiledProfileCatalogV1() {
      return Uint8Array.from(canonicalArchive);
    },
    privacyValidateCompiledProfileCatalogV1(archive) {
      return Buffer.from(archive).equals(canonicalArchive)
        ? PRIVACY_COMPILED_PROFILE_CATALOG_VALIDATION_STATUS_V1.VALID
        : PRIVACY_COMPILED_PROFILE_CATALOG_VALIDATION_STATUS_V1.MALFORMED_ARCHIVE;
    },
    ...overrides,
  };
}

test("privacy native surface contains only the local compiled-profile catalog bridge", () => {
  for (const surface of [cryptoSurface, rootSurface]) {
    assert.equal(typeof surface.privacyCompiledProfileCatalogV1, "function");
    for (const retired of [
      "privacyCapabilitiesV1",
      "privacyValidateCapabilitiesV1",
      "PRIVACY_CAPABILITY_VALIDATION_STATUS_V1",
      "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
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

test("crypto internals and the retired static capability list stay off public surfaces", () => {
  assert.equal(typeof cryptoSurface._createCryptoApi, "function");
  for (const surface of [rootSurface, rootSurface.Crypto, cryptoSubpathSurface]) {
    assert.equal("_createCryptoApi" in surface, false);
  }
  for (const surface of [
    cryptoSurface,
    rootSurface,
    rootSurface.Crypto,
    cryptoSubpathSurface,
  ]) {
    assert.equal(RETIRED_STATIC_CRYPTO_CAPABILITY_LIST in surface, false);
  }
});

test("privacy native availability requires exact ABI plus the shared typed validator", () => {
  withCryptoApi({}, (crypto) => assert.equal(crypto.isPrivacyNativeAvailable(), false));
  withCryptoApi(
    compiledProfileCatalogBinding({ connectNoritoBridgeAbiVersion: undefined }),
    (crypto) => assert.equal(crypto.isPrivacyNativeAvailable(), false),
  );
  withCryptoApi(
    compiledProfileCatalogBinding({ privacyValidateCompiledProfileCatalogV1: undefined }),
    (crypto) => assert.equal(crypto.isPrivacyNativeAvailable(), false),
  );
  withCryptoApi(
    compiledProfileCatalogBinding({
      connectNoritoBridgeAbiVersion() {
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION - 1;
      },
    }),
    (crypto) => assert.equal(crypto.isPrivacyNativeAvailable(), false),
  );
  withCryptoApi(
    compiledProfileCatalogBinding({
      connectNoritoBridgeAbiVersion() {
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION + 1;
      },
    }),
    (crypto) => assert.equal(crypto.isPrivacyNativeAvailable(), false),
  );
  withCryptoApi(compiledProfileCatalogBinding(), (crypto) => {
    assert.equal(crypto.isPrivacyNativeAvailable(), true);
  });
});

test("privacyCompiledProfileCatalogV1 returns a defensive copy of a valid Norito archive", () => {
  const nativeArchive = validCompiledProfileCatalogArchive();
  withCryptoApi(
    compiledProfileCatalogBinding({
      privacyCompiledProfileCatalogV1() {
        return nativeArchive;
      },
    }),
    (crypto) => {
      const returned = crypto.privacyCompiledProfileCatalogV1();
      assert.deepEqual(returned, validCompiledProfileCatalogArchive());
      returned.fill(0);
      assert.deepEqual(nativeArchive, validCompiledProfileCatalogArchive());
    },
  );
});

test("privacyCompiledProfileCatalogV1 rejects every output the exact local typed validator rejects", () => {
  const malformed = [
    Buffer.alloc(0),
    Buffer.from([0x50]),
    Buffer.from(validCompiledProfileCatalogArchive()),
  ];
  malformed[2][0] ^= 0xff;

  for (const archive of malformed) {
    withCryptoApi(
      compiledProfileCatalogBinding({ privacyCompiledProfileCatalogV1: () => archive }),
      (crypto) => assert.throws(() => crypto.privacyCompiledProfileCatalogV1()),
    );
  }

  const wrongSchema = validCompiledProfileCatalogArchive();
  wrongSchema.fill(0x42, 6, 22);
  withCryptoApi(
    compiledProfileCatalogBinding({ privacyCompiledProfileCatalogV1: () => wrongSchema }),
    (crypto) => assert.throws(() => crypto.privacyCompiledProfileCatalogV1(), /invalid typed privacy compiled-profile catalog/),
  );

  withCryptoApi(
    compiledProfileCatalogBinding({ privacyCompiledProfileCatalogV1: () => crcValidOneByteFake() }),
    (crypto) => assert.throws(() => crypto.privacyCompiledProfileCatalogV1(), /invalid typed privacy compiled-profile catalog/),
  );

  withCryptoApi(
    compiledProfileCatalogBinding({
      privacyCompiledProfileCatalogV1: () => Buffer.alloc(PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES + 1),
    }),
    (crypto) => assert.throws(() => crypto.privacyCompiledProfileCatalogV1(), /oversized output/),
  );
});

test("privacyCompiledProfileCatalogV1 sanitizes native exceptions", () => {
  withCryptoApi(
    compiledProfileCatalogBinding({
      privacyCompiledProfileCatalogV1() {
        throw new Error("secret native detail");
      },
    }),
    (crypto) => {
      assert.throws(
        () => crypto.privacyCompiledProfileCatalogV1(),
        (error) =>
          error.message === "native privacyCompiledProfileCatalogV1 failed" &&
          !error.message.includes("secret native detail"),
      );
    },
  );
});
