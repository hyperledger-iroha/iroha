import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  KotodamaDecimal,
  KotodamaInt,
  KotodamaQuantity,
  NumericV1,
  NumericV1Error,
} from "../src/numericV1.js";

test("numeric V1 values stay lossless and canonical", () => {
  assert.equal(new KotodamaInt(NumericV1.INT_MIN).toString(), NumericV1.INT_MIN.toString());
  assert.equal(new KotodamaInt(NumericV1.INT_MAX).toString(), NumericV1.INT_MAX.toString());
});

test("numeric V1 canonical construction and endpoint rejection", () => {
  assert.equal(new KotodamaDecimal("1.2300").toString(), "1.23");
  assert.equal(new KotodamaDecimal("0.000").toString(), "0");
  assert.equal(new KotodamaQuantity("12.50").toString(), "12.5");
  assert.throws(() => new KotodamaQuantity("-0.1"), { code: "negative_quantity" });
  assert.throws(() => new KotodamaQuantity(`-${"9".repeat(154)}`), {
    code: "mantissa_overflow",
  });
  assert.throws(() => new KotodamaInt(NumericV1.INT_MAX + 1n), { code: "mantissa_overflow" });
  assert.throws(() => new KotodamaInt(NumericV1.INT_MIN - 1n), { code: "mantissa_overflow" });
  assert.throws(() => new KotodamaInt("1".repeat(10_000)), { code: "mantissa_overflow" });
  assert.throws(() => new KotodamaInt("x".repeat(10_000)), { code: "invalid_text" });
  assert.throws(() => new KotodamaDecimal("1".repeat(10_000)), { code: "mantissa_overflow" });
  assert.throws(() => new KotodamaInt(1), TypeError);
  assert.throws(() => new KotodamaDecimal(1.5), TypeError);
  assert.equal(new KotodamaDecimal("1.00000000000000000000000000000").toString(), "1");
  assert.equal(new KotodamaDecimal(`1.${"0".repeat(10_000)}`).toString(), "1");
  assert.equal(new KotodamaDecimal(`${NumericV1.INT_MAX}.0`).toString(), NumericV1.INT_MAX.toString());
  assert.equal(
    new KotodamaDecimal(NumericV1.INT_MAX * 10n, 1).toString(),
    NumericV1.INT_MAX.toString(),
  );
  assert.throws(() => new KotodamaDecimal(`${NumericV1.INT_MAX}.1`), {
    code: "mantissa_overflow",
  });
  assert.throws(() => new KotodamaDecimal("0.00000000000000000000000000001"), {
    code: "invalid_scale",
  });
  assert.throws(() => new KotodamaDecimal("01"), { code: "invalid_text" });
  assert.equal(NumericV1.decodeDecimalJson("1.23").toString(), "1.23");
  assert.equal(NumericV1.decodeQuantityJson("0").toString(), "0");
  for (const alternate of ["+1", "01", "1.", ".5", "1e0", "-0", "-0.0", "1.0", "1.2300", "0.0"]) {
    assert.throws(() => NumericV1.decodeDecimalJson(alternate), { code: "invalid_text" });
  }
  for (const alternate of ["+1", "01", "-0", "1.0", "1e0"]) {
    assert.throws(() => NumericV1.decodeIntJson(alternate), { code: "invalid_text" });
  }
  assert.throws(() => NumericV1.decodeQuantityJson("1.0"), { code: "invalid_text" });
  assert.throws(() => NumericV1.decodeQuantityJson("-1"), { code: "negative_quantity" });
  assert.throws(() => NumericV1.decodeIntJson(1), TypeError);
});

test("numeric V1 frames and pointer envelopes roundtrip all domains", () => {
  const values = [
    [0x0011, new KotodamaInt(-129n), NumericV1.encodeIntFrame, NumericV1.decodeIntFrame,
      NumericV1.encodeIntEnvelope, NumericV1.decodeIntEnvelope],
    [0x0012, new KotodamaDecimal("-1.25"), NumericV1.encodeDecimalFrame, NumericV1.decodeDecimalFrame,
      NumericV1.encodeDecimalEnvelope, NumericV1.decodeDecimalEnvelope],
    [0x0013, new KotodamaQuantity("1.25"), NumericV1.encodeQuantityFrame, NumericV1.decodeQuantityFrame,
      NumericV1.encodeQuantityEnvelope, NumericV1.decodeQuantityEnvelope],
  ];
  for (const [pointerType, value, encodeFrame, decodeFrame, encodeEnvelope, decodeEnvelope] of values) {
    assert.equal(decodeFrame(encodeFrame(value)).toString(), value.toString());
    const envelope = encodeEnvelope(value);
    assert.deepEqual(Array.from(envelope.subarray(0, 2)), [pointerType >> 8, pointerType & 0xff]);
    assert.equal(decodeEnvelope(envelope).toString(), value.toString());
  }
  assert.throws(
    () => NumericV1.decodeDecimalEnvelope(NumericV1.encodeIntEnvelope(1n)),
    { code: "wrong_type" },
  );
});

test("numeric V1 rejects noncanonical and authenticated mutations", () => {
  const frame = NumericV1.encodeIntFrame(128n);
  for (let length = 0; length < frame.length; length += 1) {
    assert.throws(() => NumericV1.decodeIntFrame(frame.subarray(0, length)), NumericV1Error);
  }

  const badChecksum = frame.slice();
  badChecksum[badChecksum.length - 1] ^= 1;
  assert.throws(() => NumericV1.decodeIntFrame(badChecksum), { code: "checksum_mismatch" });

  const badHash = NumericV1.encodeIntEnvelope(128n).slice();
  badHash[badHash.length - 1] ^= 1;
  assert.throws(() => NumericV1.decodeIntEnvelope(badHash), { code: "payload_hash_mismatch" });

  const retired = NumericV1.encodeIntEnvelope(1n).slice();
  retired[0] = 0;
  retired[1] = 0x10;
  retired[2] = 2;
  assert.throws(() => NumericV1.decodeIntEnvelope(retired), { code: "type_not_allowed" });

  const knownWrong = NumericV1.encodeIntEnvelope(1n).slice();
  knownWrong[0] = 0;
  knownWrong[1] = 0x01;
  knownWrong[2] = 2;
  assert.throws(() => NumericV1.decodeIntEnvelope(knownWrong), { code: "wrong_type" });

  const unknown = NumericV1.encodeIntEnvelope(1n).slice();
  unknown[0] = 0;
  unknown[1] = 0x14;
  unknown[2] = 2;
  assert.throws(() => NumericV1.decodeIntEnvelope(unknown), { code: "unknown_type" });
});

test("numeric V1 consumes the Rust-authored shared golden fixture", async () => {
  const fixture = JSON.parse(await readFile(
    new URL("../../../fixtures/numeric_v1_golden.json", import.meta.url),
    "utf8",
  ));
  assert.equal(fixture.format, "iroha.numeric.v1");
  assert.equal(fixture.signed_bits, 512);
  assert.equal(fixture.maximum_scale, 28);

  for (const vector of fixture.text) {
    const decoded = vector.kind === "decimal"
      ? new KotodamaDecimal(vector.input)
      : new KotodamaQuantity(vector.input);
    assert.equal(decoded.toString(), vector.canonical, vector.id);
  }

  for (const vector of fixture.valid) {
    const value = vector.kind === "int"
      ? NumericV1.decodeIntJson(vector.canonical)
      : vector.kind === "decimal"
        ? NumericV1.decodeDecimalJson(vector.canonical)
        : NumericV1.decodeQuantityJson(vector.canonical);
    const frame = vector.kind === "int"
      ? NumericV1.encodeIntFrame(value)
      : vector.kind === "decimal"
        ? NumericV1.encodeDecimalFrame(value)
        : NumericV1.encodeQuantityFrame(value);
    const envelope = vector.kind === "int"
      ? NumericV1.encodeIntEnvelope(value)
      : vector.kind === "decimal"
        ? NumericV1.encodeDecimalEnvelope(value)
        : NumericV1.encodeQuantityEnvelope(value);
    assert.equal(toHex(frame.subarray(40)), vector.body_hex, `${vector.id} body`);
    assert.equal(toHex(frame), vector.frame_hex, `${vector.id} frame`);
    assert.equal(toHex(envelope), vector.envelope_hex, `${vector.id} envelope`);

    const fixtureFrame = fromHex(vector.frame_hex);
    const fixtureEnvelope = fromHex(vector.envelope_hex);
    const decodedFrame = vector.kind === "int"
      ? NumericV1.decodeIntFrame(fixtureFrame)
      : vector.kind === "decimal"
        ? NumericV1.decodeDecimalFrame(fixtureFrame)
        : NumericV1.decodeQuantityFrame(fixtureFrame);
    const decodedEnvelope = vector.kind === "int"
      ? NumericV1.decodeIntEnvelope(fixtureEnvelope)
      : vector.kind === "decimal"
        ? NumericV1.decodeDecimalEnvelope(fixtureEnvelope)
        : NumericV1.decodeQuantityEnvelope(fixtureEnvelope);
    assert.equal(decodedFrame.toString(), vector.canonical, `${vector.id} frame decode`);
    assert.equal(decodedEnvelope.toString(), vector.canonical, `${vector.id} envelope decode`);
  }

  for (const vector of fixture.invalid) {
    const bytes = fromHex(vector.hex);
    const decode = vector.input === "frame"
      ? vector.decode_as === "int"
        ? NumericV1.decodeIntFrame
        : vector.decode_as === "decimal"
          ? NumericV1.decodeDecimalFrame
          : NumericV1.decodeQuantityFrame
      : vector.decode_as === "int"
        ? NumericV1.decodeIntEnvelope
        : vector.decode_as === "decimal"
          ? NumericV1.decodeDecimalEnvelope
          : NumericV1.decodeQuantityEnvelope;
    assert.throws(() => decode(bytes), { code: vector.expected }, vector.id);
  }
});

function fromHex(value) {
  return Uint8Array.from(value.match(/../gu) ?? [], (byte) => Number.parseInt(byte, 16));
}

function toHex(value) {
  return Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("");
}
