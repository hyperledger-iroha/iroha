package org.hyperledger.iroha.android.client;

import java.util.LinkedHashMap;
import java.util.Map;

public final class JsonEncoderTests {

  private JsonEncoderTests() {}

  public static void main(final String[] args) {
    sortsObjectKeysLexicographically();
    System.out.println("[IrohaAndroid] JsonEncoderTests passed.");
  }

  private static void sortsObjectKeysLexicographically() {
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("z", 1);
    payload.put("a", 2);
    payload.put("m", Map.of("b", 1, "a", 2));
    final String encoded = JsonEncoder.encode(payload);
    final String expected = "{\"a\":2,\"m\":{\"a\":2,\"b\":1},\"z\":1}";
    assert expected.equals(encoded) : "JSON keys should be sorted lexicographically";
  }
}
