import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  decodeCancelAssetLockV1,
  encodeCancelAssetLockV1,
} from "../src/norito.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixtureRoot = path.resolve(
  __dirname,
  "..",
  "..",
  "..",
  "fixtures",
  "sorafs_manifest",
  "appeal_finance",
);
const requiredFixtureNames = Object.freeze([
  "cancel_asset_lock_v1.json",
  "cancel_asset_lock_v1.to",
  "negative/cancel_asset_lock_legacy_missing_expected_v1.json",
  "negative/cancel_asset_lock_legacy_missing_expected_v1.to",
  "negative/cancel_asset_lock_nested_escrow_id_v1.to",
  "negative/cancel_asset_lock_noncanonical_quantity_v1.json",
  "negative/cancel_asset_lock_zero_expected_v1.json",
  "negative/cancel_asset_lock_zero_expected_v1.to",
]);
const fixtureBytes = new Map(
  requiredFixtureNames.map((name) => [
    name,
    fs.readFileSync(path.join(fixtureRoot, name)),
  ]),
);
const canonicalEscrowId =
  "hash:73CCD4E0DD69AD434DB75056B600AA4F74C8FC5556B11BDC799DFDB7EA29851F#434B";
const canonicalArchiveHex =
  "4e5254300000b5c8a665a7de80e2eef75ccb287078fa002d00000000000000" +
  "d5f0a9bf0af707a1022073ccd4e0dd69ad434db75056b600aa4f74c8fc5556b11bdc" +
  "799dfdb7ea29851f0b0501000000140400000000";
const crc64Mask = 0xffff_ffff_ffff_ffffn;
const crc64ReflectedPolynomial = 0xc96c_5795_d787_0f42n;

function strictJsonObject(bytes) {
  const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  const value = JSON.parse(text);
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value)
  ) {
    throw new TypeError("fixture JSON must be an object");
  }
  return value;
}

function crc64(payload) {
  let crc = crc64Mask;
  for (const byte of payload) {
    crc ^= BigInt(byte);
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 1n) === 1n
          ? (crc >> 1n) ^ crc64ReflectedPolynomial
          : crc >> 1n;
    }
  }
  return BigInt.asUintN(64, crc ^ crc64Mask);
}

function archiveWithTrueTrailingPayloadByte(archive) {
  const malformed = Buffer.concat([archive, Buffer.of(0)]);
  const payload = malformed.subarray(40);
  malformed.writeBigUInt64LE(BigInt(payload.length), 23);
  malformed.writeBigUInt64LE(crc64(payload), 31);
  return malformed;
}

test("all eight appeal-finance CancelAssetLock fixtures are mandatory", () => {
  assert.equal(fixtureBytes.size, 8);
  assert.deepEqual([...fixtureBytes.keys()], requiredFixtureNames);
  for (const [name, bytes] of fixtureBytes) {
    assert.ok(bytes.length > 0, `${name} must not be empty`);
  }
});

test("bare CancelAssetLock V1 codec byte-matches and decodes the canonical fixture", () => {
  const canonicalJson = strictJsonObject(
    fixtureBytes.get("cancel_asset_lock_v1.json"),
  );
  const canonicalArchive = fixtureBytes.get("cancel_asset_lock_v1.to");
  assert.deepEqual(canonicalJson, {
    escrow_id: canonicalEscrowId,
    expected_remaining_amount: "20",
  });
  assert.equal(canonicalArchive.length, 85);
  assert.equal(canonicalArchive.toString("hex"), canonicalArchiveHex);
  assert.deepEqual(encodeCancelAssetLockV1(canonicalJson), canonicalArchive);
  assert.deepEqual(decodeCancelAssetLockV1(canonicalArchive), canonicalJson);
});

test("bare CancelAssetLock V1 rejects all shared negative fixtures", () => {
  for (const name of [
    "negative/cancel_asset_lock_legacy_missing_expected_v1.json",
    "negative/cancel_asset_lock_noncanonical_quantity_v1.json",
    "negative/cancel_asset_lock_zero_expected_v1.json",
  ]) {
    const value = strictJsonObject(fixtureBytes.get(name));
    assert.throws(
      () => encodeCancelAssetLockV1(value),
      undefined,
      `accepted ${name}`,
    );
  }
  for (const name of [
    "negative/cancel_asset_lock_legacy_missing_expected_v1.to",
    "negative/cancel_asset_lock_nested_escrow_id_v1.to",
    "negative/cancel_asset_lock_zero_expected_v1.to",
  ]) {
    assert.throws(
      () => decodeCancelAssetLockV1(fixtureBytes.get(name)),
      undefined,
      `accepted ${name}`,
    );
  }
});

test("bare CancelAssetLock V1 encoder rejects aliases and noncanonical structures", () => {
  const canonical = {
    escrow_id: canonicalEscrowId,
    expected_remaining_amount: "20",
  };
  for (const value of [
    null,
    [],
    [canonical],
    { CancelAssetLock: canonical },
    { ...canonical, extra: true },
    { escrow_id: canonicalEscrowId },
    { expected_remaining_amount: "20" },
    { escrowId: canonicalEscrowId, expectedRemainingAmount: "20" },
  ]) {
    assert.throws(() => encodeCancelAssetLockV1(value));
  }

  for (const escrow_id of [
    canonicalEscrowId.slice(5, 69),
    Buffer.from(canonicalEscrowId.slice(5, 69), "hex"),
    Buffer.from(canonicalEscrowId.slice(5, 69), "hex").toString("base64"),
    [canonicalEscrowId],
    { Hash: canonicalEscrowId },
    canonicalEscrowId.toLowerCase(),
    `${canonicalEscrowId.slice(0, -1)}0`,
    "\ud800",
    "\udc00",
  ]) {
    assert.throws(
      () =>
        encodeCancelAssetLockV1({
          escrow_id,
          expected_remaining_amount: "20",
        }),
      undefined,
      `accepted escrow_id alias ${String(escrow_id)}`,
    );
  }

  for (const expected_remaining_amount of [
    20,
    20n,
    Buffer.from("20"),
    ["20"],
    { Quantity: "20" },
    "",
    "0",
    "-1",
    "020",
    "+20",
    "20.0",
    "2e1",
    "\ud800",
    "\udc00",
  ]) {
    assert.throws(
      () =>
        encodeCancelAssetLockV1({
          escrow_id: canonicalEscrowId,
          expected_remaining_amount,
        }),
      undefined,
      `accepted quantity alias ${String(expected_remaining_amount)}`,
    );
  }
});

test("bare CancelAssetLock V1 decoder accepts bytes only and rejects frame substitution", () => {
  const canonical = fixtureBytes.get("cancel_asset_lock_v1.to");
  for (const alias of [
    canonical.toString("hex"),
    canonical.toString("base64"),
    [...canonical],
    { bytes: canonical },
  ]) {
    assert.throws(() => decodeCancelAssetLockV1(alias));
  }

  const wrongVersion = Buffer.from(canonical);
  wrongVersion[4] = 1;
  assert.throws(() => decodeCancelAssetLockV1(wrongVersion), /version/u);

  const wrongSchema = Buffer.from(canonical);
  wrongSchema[6] ^= 1;
  assert.throws(() => decodeCancelAssetLockV1(wrongSchema), /schema/u);

  const compressed = Buffer.from(canonical);
  compressed[22] = 1;
  assert.throws(() => decodeCancelAssetLockV1(compressed), /uncompressed/u);

  const wrongFlags = Buffer.from(canonical);
  wrongFlags[39] = 0;
  assert.throws(() => decodeCancelAssetLockV1(wrongFlags), /compact-length/u);

  const padded = Buffer.concat([
    canonical.subarray(0, 40),
    Buffer.of(0),
    canonical.subarray(40),
  ]);
  assert.throws(() => decodeCancelAssetLockV1(padded), /padding/u);
});

test("nested EscrowId and true trailing payload bytes are independent failures", () => {
  const nested = fixtureBytes.get(
    "negative/cancel_asset_lock_nested_escrow_id_v1.to",
  );
  assert.equal(nested.length, 86);
  assert.deepEqual([...nested.subarray(40, 42)], [0x21, 0x20]);
  assert.throws(() => decodeCancelAssetLockV1(nested));

  const trailing = archiveWithTrueTrailingPayloadByte(
    fixtureBytes.get("cancel_asset_lock_v1.to"),
  );
  assert.equal(trailing.length, 86);
  assert.equal(trailing.readBigUInt64LE(23), 46n);
  assert.throws(() => decodeCancelAssetLockV1(trailing), /trailing bytes/u);
});

test("fixture JSON decoding is fatal for malformed UTF-8", () => {
  assert.throws(
    () => strictJsonObject(Buffer.from([0x7b, 0x22, 0x80, 0x22, 0x3a, 0x31, 0x7d])),
    /encoded data|encoding/u,
  );
});
