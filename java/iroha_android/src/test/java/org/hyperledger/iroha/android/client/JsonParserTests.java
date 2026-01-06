package org.hyperledger.iroha.android.client;

public final class JsonParserTests {

  private JsonParserTests() {}

  public static void main(final String[] args) {
    parsesNumbers();
    rejectsLeadingZeros();
    rejectsOverflow();
    System.out.println("[IrohaAndroid] JsonParserTests passed.");
  }

  private static void parsesNumbers() {
    final Object zero = JsonParser.parse("0");
    assert zero instanceof Long && ((Long) zero) == 0L : "expected integer zero";
    final Object exponent = JsonParser.parse("1e3");
    assert exponent instanceof Double && ((Double) exponent) == 1000.0 : "expected exponent value";
    final Object fraction = JsonParser.parse("-0.5");
    assert fraction instanceof Double && ((Double) fraction) == -0.5 : "expected fraction value";
  }

  private static void rejectsLeadingZeros() {
    assertThrows(() -> JsonParser.parse("01"), "expected leading-zero rejection");
    assertThrows(() -> JsonParser.parse("-01"), "expected leading-zero rejection");
  }

  private static void rejectsOverflow() {
    assertThrows(() -> JsonParser.parse("1e309"), "expected overflow rejection");
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
