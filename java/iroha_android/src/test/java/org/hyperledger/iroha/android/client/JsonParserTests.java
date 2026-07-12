package org.hyperledger.iroha.android.client;

import java.math.BigDecimal;
import java.math.BigInteger;

public final class JsonParserTests {

  private JsonParserTests() {}

  public static void main(final String[] args) {
    parsesNumbers();
    checkedLongCoercionIsExactAndBounded();
    rejectsLeadingZeros();
    preservesDecimalAndExponentTokensExactly();
    oversizedIntegerTokensRemainAvailableForBigIntegerConsumers();
    rejectsDuplicateObjectKeys();
    validatesStringControlsAndUnicodeScalars();
    boundsNestingBeforeTheRuntimeStack();
    System.out.println("[IrohaAndroid] JsonParserTests passed.");
  }

  private static void checkedLongCoercionIsExactAndBounded() {
    assert JsonNumbers.asLong(JsonParser.parse(Long.MAX_VALUE + ""), "max") == Long.MAX_VALUE;
    assert JsonNumbers.asLong(JsonParser.parse(Long.MIN_VALUE + ""), "min") == Long.MIN_VALUE;
    assertThrows(
        () -> JsonNumbers.asLong(JsonParser.parse("9223372036854775808"), "height"),
        "expected positive integer overflow rejection");
    assertThrows(
        () -> JsonNumbers.asLong(JsonParser.parse("-9223372036854775809"), "height"),
        "expected negative integer overflow rejection");
    assertThrows(
        () -> JsonNumbers.asLong(JsonParser.parse("1.0"), "height"),
        "expected decimal integer token rejection");
    assertThrows(
        () -> JsonNumbers.asLong(JsonParser.parse("1e0"), "height"),
        "expected exponent integer token rejection");
    assert JsonNumbers.asLongAllowingIntegralFloat(JsonParser.parse("1e0"), "height") == 1L;
    assertThrows(
        () -> JsonNumbers.asLongAllowingIntegralFloat(JsonParser.parse("1.5"), "height"),
        "expected non-integral decimal rejection");
  }

  private static void parsesNumbers() {
    final Object zero = JsonParser.parse("0");
    assert zero instanceof Long && ((Long) zero) == 0L : "expected integer zero";
    final Object exponent = JsonParser.parse("1e3");
    assert new BigDecimal("1e3").equals(exponent) : "expected exact exponent value";
    final Object fraction = JsonParser.parse("-0.5");
    assert new BigDecimal("-0.5").equals(fraction) : "expected exact fraction value";
  }

  private static void rejectsLeadingZeros() {
    assertThrows(() -> JsonParser.parse("01"), "expected leading-zero rejection");
    assertThrows(() -> JsonParser.parse("-01"), "expected leading-zero rejection");
  }

  private static void preservesDecimalAndExponentTokensExactly() {
    assert new BigDecimal("1e400").equals(JsonParser.parse("1e400"))
        : "large exponent must remain exact";
  }

  private static void oversizedIntegerTokensRemainAvailableForBigIntegerConsumers() {
    final String raw = "184467440737095516160000000000000000000";
    final Object parsed = JsonParser.parse(raw);
    assert parsed instanceof BigInteger : "expected BigInteger";
    assert new BigInteger(raw).equals(parsed) : "BigInteger value mismatch";
  }

  private static void rejectsDuplicateObjectKeys() {
    assertThrows(
        () -> JsonParser.parse("{\"bundle_id\":\"forged\",\"bundle_id\":\"trusted\"}"),
        "expected duplicate key rejection");
    assertThrows(
        () -> JsonParser.parse("{\"outer\":{\"key\":1,\"key\":2}}"),
        "expected nested duplicate key rejection");
    assertThrows(
        () -> JsonParser.parse("{\"bundle\\u005fid\":\"forged\",\"bundle_id\":\"trusted\"}"),
        "expected escaped duplicate key rejection");
  }

  private static void validatesStringControlsAndUnicodeScalars() {
    assert "emoji: 😀".equals(JsonParser.parse("\"emoji: \\uD83D\\uDE00\""));
    assert "emoji: 😀".equals(JsonParser.parse("\"emoji: 😀\""));
    assertThrows(() -> JsonParser.parse("\"raw\u0001control\""), "expected raw control rejection");
    assertThrows(() -> JsonParser.parse("\"\\uD800\""), "expected escaped high surrogate rejection");
    assertThrows(
        () -> JsonParser.parse("\"" + new String(new char[] {'\uD800'}) + "\""),
        "expected raw high surrogate rejection");
    assertThrows(() -> JsonParser.parse("\"\\uDC00\""), "expected low surrogate rejection");
  }

  private static void boundsNestingBeforeTheRuntimeStack() {
    JsonParser.parse("[".repeat(128) + "0" + "]".repeat(128));
    assertThrows(
        () -> JsonParser.parse("[".repeat(129) + "0" + "]".repeat(129)),
        "expected nesting depth rejection");
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final IllegalStateException expected) {
      return;
    }
    throw new AssertionError(message);
  }
}
