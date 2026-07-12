package org.hyperledger.iroha.android.numeric;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;
import org.junit.Test;

public final class NumericV1Tests {
  @Test
  public void exactValuesCanonicalizeWithoutLossyHostNumbers() {
    assertEquals(NumericV1.INT_MIN.toString(), NumericV1.IntValue.of(NumericV1.INT_MIN).toString());
    assertEquals(NumericV1.INT_MAX.toString(), NumericV1.IntValue.of(NumericV1.INT_MAX).toString());
    assertEquals("1.23", NumericV1.DecimalValue.parse("1.2300").toString());
    assertEquals("0", NumericV1.DecimalValue.parse("0.000").toString());
    assertEquals("12.5", NumericV1.QuantityValue.parse("12.50").toString());
    assertEquals("12.5", NumericV1.QuantityValue.parseCanonical("12.5").toString());
    for (final String alternate :
        new String[] {"", " ", "\t1", "1 ", "+1", "01", "1.", ".5", "1e0", "-0", "-0.0", "1.0", "1.2300", "0.0"}) {
      assertCode(
          NumericV1.ErrorCode.INVALID_TEXT,
          () -> NumericV1.QuantityValue.parseCanonical(alternate));
    }
    assertCode(
        NumericV1.ErrorCode.NEGATIVE_QUANTITY,
        () -> NumericV1.QuantityValue.parseCanonical("-1"));
    assertCode(NumericV1.ErrorCode.NEGATIVE_QUANTITY, () -> NumericV1.QuantityValue.parse("-0.1"));
    assertCode(
        NumericV1.ErrorCode.MANTISSA_OVERFLOW,
        () -> NumericV1.QuantityValue.parse("-" + repeat("9", 154)));
    assertCode(
        NumericV1.ErrorCode.MANTISSA_OVERFLOW,
        () -> NumericV1.IntValue.of(NumericV1.INT_MAX.add(BigInteger.ONE)));
    assertCode(
        NumericV1.ErrorCode.MANTISSA_OVERFLOW,
        () -> NumericV1.IntValue.of(NumericV1.INT_MIN.subtract(BigInteger.ONE)));
    assertCode(
        NumericV1.ErrorCode.MANTISSA_OVERFLOW,
        () -> NumericV1.IntValue.parse(repeat("1", 10_000)));
    assertCode(
        NumericV1.ErrorCode.INVALID_TEXT,
        () -> NumericV1.IntValue.parse(repeat("x", 10_000)));
    assertCode(
        NumericV1.ErrorCode.MANTISSA_OVERFLOW,
        () -> NumericV1.DecimalValue.parse(repeat("1", 10_000)));
    assertEquals("1", NumericV1.DecimalValue.parse("1.00000000000000000000000000000").toString());
    assertEquals("1", NumericV1.DecimalValue.parse("1." + repeat("0", 10_000)).toString());
    assertEquals(
        NumericV1.INT_MAX.toString(),
        NumericV1.DecimalValue.parse(NumericV1.INT_MAX + ".0").toString());
    assertEquals(
        NumericV1.INT_MAX.toString(),
        NumericV1.DecimalValue.of(NumericV1.INT_MAX.multiply(BigInteger.TEN), 1).toString());
    assertCode(
        NumericV1.ErrorCode.MANTISSA_OVERFLOW,
        () -> NumericV1.DecimalValue.parse(NumericV1.INT_MAX + ".1"));
    assertCode(
        NumericV1.ErrorCode.INVALID_SCALE,
        () -> NumericV1.DecimalValue.parse("0.00000000000000000000000000001"));
    assertCode(NumericV1.ErrorCode.INVALID_TEXT, () -> NumericV1.IntValue.parse("01"));
    assertEquals("1.23", NumericV1.decodeDecimalJson("1.23").toString());
    assertEquals("0", NumericV1.decodeQuantityJson("0").toString());
    for (final String alternate :
        new String[] {"+1", "01", "1.", ".5", "1e0", "-0", "-0.0", "1.0", "1.2300", "0.0"}) {
      assertCode(NumericV1.ErrorCode.INVALID_TEXT, () -> NumericV1.decodeDecimalJson(alternate));
    }
    for (final String alternate : new String[] {"+1", "01", "-0", "1.0", "1e0"}) {
      assertCode(NumericV1.ErrorCode.INVALID_TEXT, () -> NumericV1.decodeIntJson(alternate));
    }
    assertCode(NumericV1.ErrorCode.INVALID_TEXT, () -> NumericV1.decodeQuantityJson("1.0"));
    assertCode(NumericV1.ErrorCode.NEGATIVE_QUANTITY, () -> NumericV1.decodeQuantityJson("-1"));
  }

  @Test
  public void canonicalFramesAndEnvelopesRoundtrip() {
    final NumericV1.IntValue integer = NumericV1.IntValue.parse("-129");
    final byte[] integerEnvelope = NumericV1.encodeIntEnvelope(integer);
    assertEquals(0x00, integerEnvelope[0] & 0xFF);
    assertEquals(0x11, integerEnvelope[1] & 0xFF);
    assertEquals(integer, NumericV1.decodeIntFrame(NumericV1.encodeIntFrame(integer)));
    assertEquals(integer, NumericV1.decodeIntEnvelope(integerEnvelope));

    final NumericV1.DecimalValue decimal = NumericV1.DecimalValue.parse("-1.25");
    final byte[] decimalEnvelope = NumericV1.encodeDecimalEnvelope(decimal);
    assertEquals(0x00, decimalEnvelope[0] & 0xFF);
    assertEquals(0x12, decimalEnvelope[1] & 0xFF);
    assertEquals(decimal, NumericV1.decodeDecimalFrame(NumericV1.encodeDecimalFrame(decimal)));
    assertEquals(decimal, NumericV1.decodeDecimalEnvelope(decimalEnvelope));

    final NumericV1.QuantityValue quantity = NumericV1.QuantityValue.parse("1.25");
    final byte[] quantityEnvelope = NumericV1.encodeQuantityEnvelope(quantity);
    assertEquals(0x00, quantityEnvelope[0] & 0xFF);
    assertEquals(0x13, quantityEnvelope[1] & 0xFF);
    assertEquals(quantity, NumericV1.decodeQuantityFrame(NumericV1.encodeQuantityFrame(quantity)));
    assertEquals(quantity, NumericV1.decodeQuantityEnvelope(quantityEnvelope));

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
    retired[2] = 2;
    assertCode(NumericV1.ErrorCode.TYPE_NOT_ALLOWED, () -> NumericV1.decodeIntEnvelope(retired));

    final byte[] knownWrong = NumericV1.encodeIntEnvelope(NumericV1.IntValue.parse("1"));
    knownWrong[0] = 0;
    knownWrong[1] = 0x01;
    knownWrong[2] = 2;
    assertCode(NumericV1.ErrorCode.WRONG_TYPE, () -> NumericV1.decodeIntEnvelope(knownWrong));

    final byte[] unknown = NumericV1.encodeIntEnvelope(NumericV1.IntValue.parse("1"));
    unknown[0] = 0;
    unknown[1] = 0x14;
    unknown[2] = 2;
    assertCode(NumericV1.ErrorCode.UNKNOWN_TYPE, () -> NumericV1.decodeIntEnvelope(unknown));
  }

  @Test
  @SuppressWarnings("unchecked")
  public void consumesRustAuthoredSharedGoldenFixture() throws Exception {
    final String text = new String(Files.readAllBytes(sharedFixture()), StandardCharsets.UTF_8);
    final Map<String, Object> fixture = (Map<String, Object>) JsonParser.parse(text);
    assertEquals("iroha.numeric.v1", fixture.get("format"));
    assertEquals(512, ((Number) fixture.get("signed_bits")).intValue());
    assertEquals(28, ((Number) fixture.get("maximum_scale")).intValue());

    for (final Object raw : (List<Object>) fixture.get("text")) {
      final Map<String, Object> vector = (Map<String, Object>) raw;
      final String input = (String) vector.get("input");
      final String canonical;
      if ("decimal".equals(vector.get("kind"))) canonical = NumericV1.DecimalValue.parse(input).toString();
      else if ("quantity".equals(vector.get("kind"))) canonical = NumericV1.QuantityValue.parse(input).toString();
      else throw new AssertionError("unknown text fixture kind " + vector.get("kind"));
      assertEquals(vector.get("id").toString(), vector.get("canonical"), canonical);
    }

    for (final Object raw : (List<Object>) fixture.get("valid")) {
      final Map<String, Object> vector = (Map<String, Object>) raw;
      final String id = (String) vector.get("id");
      final String kind = (String) vector.get("kind");
      final String canonical = (String) vector.get("canonical");
      final byte[] fixtureFrame = unhex((String) vector.get("frame_hex"));
      final byte[] fixtureEnvelope = unhex((String) vector.get("envelope_hex"));
      final byte[] frame;
      final byte[] envelope;
      final String decodedFrame;
      final String decodedEnvelope;
      switch (kind) {
        case "int":
          final NumericV1.IntValue integer = NumericV1.decodeIntJson(canonical);
          frame = NumericV1.encodeIntFrame(integer);
          envelope = NumericV1.encodeIntEnvelope(integer);
          decodedFrame = NumericV1.decodeIntFrame(fixtureFrame).toString();
          decodedEnvelope = NumericV1.decodeIntEnvelope(fixtureEnvelope).toString();
          break;
        case "decimal":
          final NumericV1.DecimalValue decimal = NumericV1.decodeDecimalJson(canonical);
          frame = NumericV1.encodeDecimalFrame(decimal);
          envelope = NumericV1.encodeDecimalEnvelope(decimal);
          decodedFrame = NumericV1.decodeDecimalFrame(fixtureFrame).toString();
          decodedEnvelope = NumericV1.decodeDecimalEnvelope(fixtureEnvelope).toString();
          break;
        case "quantity":
          final NumericV1.QuantityValue quantity = NumericV1.decodeQuantityJson(canonical);
          frame = NumericV1.encodeQuantityFrame(quantity);
          envelope = NumericV1.encodeQuantityEnvelope(quantity);
          decodedFrame = NumericV1.decodeQuantityFrame(fixtureFrame).toString();
          decodedEnvelope = NumericV1.decodeQuantityEnvelope(fixtureEnvelope).toString();
          break;
        default:
          throw new AssertionError("unknown fixture kind " + kind);
      }
      assertEquals(id + " body", vector.get("body_hex"), hex(java.util.Arrays.copyOfRange(frame, 40, frame.length)));
      assertEquals(id + " frame", vector.get("frame_hex"), hex(frame));
      assertEquals(id + " envelope", vector.get("envelope_hex"), hex(envelope));
      assertEquals(id + " frame decode", canonical, decodedFrame);
      assertEquals(id + " envelope decode", canonical, decodedEnvelope);
    }

    for (final Object raw : (List<Object>) fixture.get("invalid")) {
      final Map<String, Object> vector = (Map<String, Object>) raw;
      final String input = (String) vector.get("input");
      final String decodeAs = (String) vector.get("decode_as");
      final NumericV1.ErrorCode expected = NumericV1.ErrorCode.valueOf(
          ((String) vector.get("expected")).toUpperCase(java.util.Locale.ROOT));
      final byte[] bytes = unhex((String) vector.get("hex"));
      assertCode(expected, () -> {
        if ("frame".equals(input) && "int".equals(decodeAs)) NumericV1.decodeIntFrame(bytes);
        else if ("frame".equals(input) && "decimal".equals(decodeAs)) NumericV1.decodeDecimalFrame(bytes);
        else if ("frame".equals(input) && "quantity".equals(decodeAs)) NumericV1.decodeQuantityFrame(bytes);
        else if ("envelope".equals(input) && "int".equals(decodeAs)) NumericV1.decodeIntEnvelope(bytes);
        else if ("envelope".equals(input) && "decimal".equals(decodeAs)) NumericV1.decodeDecimalEnvelope(bytes);
        else if ("envelope".equals(input) && "quantity".equals(decodeAs)) NumericV1.decodeQuantityEnvelope(bytes);
        else throw new AssertionError("unknown fixture decoder " + input + "/" + decodeAs);
      });
    }

    for (final Object raw : (List<Object>) fixture.get("invalid_text")) {
      final Map<String, Object> vector = (Map<String, Object>) raw;
      final String kind = (String) vector.get("kind");
      final Object input = vector.get("input");
      final NumericV1.ErrorCode expected =
          NumericV1.ErrorCode.valueOf(
              ((String) vector.get("expected")).toUpperCase(java.util.Locale.ROOT));
      assertCode(
          expected,
          () -> {
            if ("int".equals(kind)) NumericV1.decodeIntJsonValue(input);
            else if ("decimal".equals(kind)) NumericV1.decodeDecimalJsonValue(input);
            else if ("quantity".equals(kind)) NumericV1.decodeQuantityJsonValue(input);
            else throw new AssertionError("unknown invalid text fixture kind " + kind);
          });
    }
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

  private static Path sharedFixture() {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve("fixtures/numeric_v1_golden.json");
      if (Files.isRegularFile(candidate)) return candidate;
      current = current.getParent();
    }
    throw new AssertionError("fixtures/numeric_v1_golden.json was not found");
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder result = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) result.append(String.format(java.util.Locale.ROOT, "%02x", value & 0xFF));
    return result.toString();
  }

  private static byte[] unhex(final String value) {
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      result[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return result;
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder result = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) result.append(value);
    return result.toString();
  }
}
