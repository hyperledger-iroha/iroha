package org.hyperledger.iroha.android.numeric;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;

import java.math.BigInteger;
import org.junit.Test;

public final class NumericV1Tests {
  @Test
  public void exactValuesCanonicalizeWithoutLossyHostNumbers() {
    assertEquals(NumericV1.INT_MIN.toString(), NumericV1.IntValue.of(NumericV1.INT_MIN).toString());
    assertEquals(NumericV1.INT_MAX.toString(), NumericV1.IntValue.of(NumericV1.INT_MAX).toString());
    assertEquals("1.23", NumericV1.DecimalValue.parse("1.2300").toString());
    assertEquals("0", NumericV1.DecimalValue.parse("0.000").toString());
    assertEquals("12.5", NumericV1.QuantityValue.parse("12.50").toString());
    assertCode(NumericV1.ErrorCode.NEGATIVE_QUANTITY, () -> NumericV1.QuantityValue.parse("-0.1"));
    assertCode(
        NumericV1.ErrorCode.MANTISSA_OVERFLOW,
        () -> NumericV1.IntValue.of(NumericV1.INT_MAX.add(BigInteger.ONE)));
    assertCode(
        NumericV1.ErrorCode.INVALID_SCALE,
        () -> NumericV1.DecimalValue.parse("1.00000000000000000000000000000"));
    assertCode(NumericV1.ErrorCode.INVALID_TEXT, () -> NumericV1.IntValue.parse("01"));
  }

  @Test
  public void canonicalFramesAndEnvelopesRoundtrip() {
    final NumericV1.IntValue integer = NumericV1.IntValue.parse("-129");
    assertEquals(integer, NumericV1.decodeIntFrame(NumericV1.encodeIntFrame(integer)));
    assertEquals(integer, NumericV1.decodeIntEnvelope(NumericV1.encodeIntEnvelope(integer)));

    final NumericV1.DecimalValue decimal = NumericV1.DecimalValue.parse("-1.25");
    assertEquals(decimal, NumericV1.decodeDecimalFrame(NumericV1.encodeDecimalFrame(decimal)));
    assertEquals(decimal, NumericV1.decodeDecimalEnvelope(NumericV1.encodeDecimalEnvelope(decimal)));

    final NumericV1.QuantityValue quantity = NumericV1.QuantityValue.parse("1.25");
    assertEquals(quantity, NumericV1.decodeQuantityFrame(NumericV1.encodeQuantityFrame(quantity)));
    assertEquals(quantity, NumericV1.decodeQuantityEnvelope(NumericV1.encodeQuantityEnvelope(quantity)));

    assertCode(
        NumericV1.ErrorCode.WRONG_TYPE,
        () -> NumericV1.decodeDecimalEnvelope(NumericV1.encodeIntEnvelope(NumericV1.IntValue.parse("1"))));
  }

  @Test
  public void malformedAuthenticatedInputsAreRejected() {
    final byte[] frame = NumericV1.encodeIntFrame(NumericV1.IntValue.parse("128"));
    for (int length = 0; length < frame.length; length++) {
      final byte[] truncated = java.util.Arrays.copyOf(frame, length);
      assertAnyFailure(() -> NumericV1.decodeIntFrame(truncated));
    }

    final byte[] badChecksum = frame.clone();
    badChecksum[badChecksum.length - 1] ^= 1;
    assertCode(NumericV1.ErrorCode.CHECKSUM_MISMATCH, () -> NumericV1.decodeIntFrame(badChecksum));

    final byte[] badHash = NumericV1.encodeIntEnvelope(NumericV1.IntValue.parse("1"));
    badHash[badHash.length - 1] ^= 1;
    assertCode(NumericV1.ErrorCode.PAYLOAD_HASH_MISMATCH, () -> NumericV1.decodeIntEnvelope(badHash));

    final byte[] retired = NumericV1.encodeIntEnvelope(NumericV1.IntValue.parse("1"));
    retired[0] = 0;
    retired[1] = 0x10;
    assertCode(NumericV1.ErrorCode.TYPE_NOT_ALLOWED, () -> NumericV1.decodeIntEnvelope(retired));
  }

  private static void assertAnyFailure(final CheckedRunnable runnable) {
    NumericV1.NumericException failure = null;
    try {
      runnable.run();
    } catch (final NumericV1.NumericException exception) {
      failure = exception;
    }
    assertNotNull("expected strict decoder failure", failure);
  }

  private static void assertCode(
      final NumericV1.ErrorCode expected, final CheckedRunnable runnable) {
    NumericV1.NumericException failure = null;
    try {
      runnable.run();
    } catch (final NumericV1.NumericException exception) {
      failure = exception;
    }
    assertNotNull("expected strict decoder failure", failure);
    assertEquals(expected, failure.code());
  }

  private interface CheckedRunnable {
    void run();
  }
}
