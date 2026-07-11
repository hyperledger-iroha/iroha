import assert from "node:assert/strict";
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
  assert.throws(() => new KotodamaInt(NumericV1.INT_MAX + 1n), { code: "mantissa_overflow" });
  assert.throws(() => new KotodamaInt(1), TypeError);
  assert.throws(() => new KotodamaDecimal(1.5), TypeError);
  assert.throws(() => new KotodamaDecimal("1.00000000000000000000000000000"), {
    code: "invalid_scale",
  });
  assert.throws(() => new KotodamaDecimal("01"), { code: "invalid_text" });
  assert.equal(NumericV1.decodeDecimalJson("1.23").toString(), "1.23");
  assert.equal(NumericV1.decodeQuantityJson("0").toString(), "0");
  for (const alternate of ["1.0", "1.2300", "0.0"]) {
    assert.throws(() => NumericV1.decodeDecimalJson(alternate), { code: "invalid_text" });
  }
  assert.throws(() => NumericV1.decodeQuantityJson("1.0"), { code: "invalid_text" });
  assert.throws(() => NumericV1.decodeIntJson(1), TypeError);
});

test("numeric V1 frames and pointer envelopes roundtrip all domains", () => {
  const values = [
    [new KotodamaInt(-129n), NumericV1.encodeIntFrame, NumericV1.decodeIntFrame,
      NumericV1.encodeIntEnvelope, NumericV1.decodeIntEnvelope],
    [new KotodamaDecimal("-1.25"), NumericV1.encodeDecimalFrame, NumericV1.decodeDecimalFrame,
      NumericV1.encodeDecimalEnvelope, NumericV1.decodeDecimalEnvelope],
    [new KotodamaQuantity("1.25"), NumericV1.encodeQuantityFrame, NumericV1.decodeQuantityFrame,
      NumericV1.encodeQuantityEnvelope, NumericV1.decodeQuantityEnvelope],
  ];
  for (const [value, encodeFrame, decodeFrame, encodeEnvelope, decodeEnvelope] of values) {
    assert.equal(decodeFrame(encodeFrame(value)).toString(), value.toString());
    assert.equal(decodeEnvelope(encodeEnvelope(value)).toString(), value.toString());
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
  assert.throws(() => NumericV1.decodeIntEnvelope(retired), { code: "type_not_allowed" });
});
