package org.hyperledger.iroha.android.model;

import java.util.LinkedHashMap;
import java.util.Map;

/** Canonical signed-metadata JSON boundary regressions. */
public final class JsonValueTests {

  private JsonValueTests() {}

  public static void main(final String[] args) {
    canonicalizesTextConstruction();
    ordersKeysByUnicodeScalar();
    rejectsAlternateSignedWireSpellings();
    rejectsInvalidAndOutOfRangeJson();
    System.out.println("[IrohaAndroid] JsonValueTests passed.");
  }

  private static void canonicalizesTextConstruction() {
    final Map<String, String> cases = new LinkedHashMap<>();
    cases.put("1 ", "1");
    cases.put("{\"z\":0,\"a\":1}", "{\"a\":1,\"z\":0}");
    cases.put("\"\\u0061\"", "\"a\"");
    cases.put("\"\\u0008\\u000c\"", "\"\\b\\f\"");
    cases.put("1e0", "1.0");
    cases.put("-0", "-0.0");
    cases.put("1e20", "1e+20");
    cases.put("5e-324", "5e-324");
    for (final Map.Entry<String, String> entry : cases.entrySet()) {
      assert entry.getValue().equals(JsonValue.parse(entry.getKey()).canonicalJson()) : entry.getKey();
    }
  }

  private static void ordersKeysByUnicodeScalar() {
    assert "{\"\uE000\":2,\"\uD800\uDC00\":1}"
        .equals(JsonValue.parse("{\"\uD800\uDC00\":1,\"\uE000\":2}").canonicalJson());
  }

  private static void rejectsAlternateSignedWireSpellings() {
    assert "{\"a\":1}".equals(JsonValue.fromCanonicalWire("{\"a\":1}").canonicalJson());
    for (final String alternate :
        new String[] {"1 ", "{\"z\":0,\"a\":1}", "1e0", "-0"}) {
      expectFailure(() -> JsonValue.fromCanonicalWire(alternate), alternate);
    }
  }

  private static void rejectsInvalidAndOutOfRangeJson() {
    for (final String invalid :
        new String[] {"", "plain", "{\"a\":1,\"a\":2}", "1e400"}) {
      expectFailure(() -> JsonValue.parse(invalid), invalid);
    }
  }

  private static void expectFailure(final Runnable operation, final String input) {
    try {
      operation.run();
    } catch (IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError("expected JsonValue rejection: " + input);
  }
}
