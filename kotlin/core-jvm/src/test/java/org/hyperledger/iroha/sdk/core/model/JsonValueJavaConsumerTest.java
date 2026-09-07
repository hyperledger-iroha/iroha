package org.hyperledger.iroha.sdk.core.model;

import static org.junit.jupiter.api.Assertions.*;

import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.Map;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.hyperledger.iroha.sdk.address.AccountAddress;
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter;
import org.junit.jupiter.api.Test;

/** Java applications author canonical, immutable metadata through the Kotlin API. */
class JsonValueJavaConsumerTest {
  @Test
  void allCanonicalFactoriesAreJavaCallable() {
    assertEquals("\"line\\n\\\"quoted\\\"\"", JsonValue.string("line\n\"quoted\"").getCanonicalJson());
    assertEquals(Long.toString(Long.MIN_VALUE), JsonValue.number(Long.MIN_VALUE).getCanonicalJson());
    assertEquals("true", JsonValue.bool(true).getCanonicalJson());
    assertEquals("false", JsonValue.bool(false).getCanonicalJson());
    assertEquals("null", JsonValue.nullValue().getCanonicalJson());
    assertEquals("{\"a\":1,\"b\":[true,null]}",
        JsonValue.parse(" { \"b\": [true, null], \"a\": 1 } ").getCanonicalJson());
  }

  @Test
  void valueEqualityUsesCanonicalJsonAndWorksInJavaCollections() {
    JsonValue first = JsonValue.parse(" {\"b\":true, \"a\":1} ");
    JsonValue second = JsonValue.parse("{\"a\":1,\"b\":true}");
    assertEquals(first, second);
    assertEquals(first.hashCode(), second.hashCode());
    assertEquals(1, new HashSet<>(Arrays.asList(first, second)).size());
    assertEquals(first.getCanonicalJson(), first.toString());
    assertNotEquals(first, first.getCanonicalJson());
    assertNotEquals(first, null);
    assertNotEquals(JsonValue.number(1), JsonValue.string("1"));
    assertNotEquals(JsonValue.number(1), JsonValue.parse("1.0"));
  }

  @Test
  void malformedAndNonfiniteJsonAreRejectedAtConstruction() {
    for (String invalid : Arrays.asList("", "null true", "{", "NaN", "Infinity", "1e9999")) {
      assertThrows(IllegalArgumentException.class, () -> JsonValue.parse(invalid), invalid);
    }
  }

  @Test
  void metadataRoundtripsAndCannotChangeAfterPayloadConstruction() throws Exception {
    Map<String, JsonValue> metadata = new LinkedHashMap<>();
    metadata.put("object", JsonValue.parse("{\"z\":2,\"a\":1}"));
    metadata.put("message", JsonValue.string("Java consumer"));
    metadata.put("missing", JsonValue.nullValue());
    Map<String, JsonValue> expected = new LinkedHashMap<>(metadata);
    TransactionPayload payload = payload(metadata);
    NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    byte[] encoded = adapter.encodeTransaction(payload);
    metadata.clear();
    metadata.put("injected", JsonValue.bool(true));
    assertEquals(expected, payload.getMetadata());
    assertThrows(UnsupportedOperationException.class,
        () -> payload.getMetadata().put("injected", JsonValue.number(2)));
    assertThrows(UnsupportedOperationException.class, () -> payload.getMetadata().remove("message"));
    assertThrows(UnsupportedOperationException.class,
        () -> payload.getMetadata().entrySet().iterator().next().setValue(JsonValue.bool(false)));
    assertArrayEquals(encoded, adapter.encodeTransaction(payload));
    assertEquals(expected, adapter.decodeTransaction(encoded).getMetadata());
  }

  @Test
  void javaNullIsNotJsonNull() throws Exception {
    Map<String, JsonValue> metadata = new LinkedHashMap<>();
    metadata.put("value", null);
    assertThrows(IllegalArgumentException.class, () -> payload(metadata));
    metadata.clear();
    metadata.put(null, JsonValue.nullValue());
    assertThrows(IllegalArgumentException.class, () -> payload(metadata));
    assertEquals(JsonValue.nullValue(), payload(Collections.singletonMap("value", JsonValue.nullValue()))
        .getMetadata().get("value"));
  }

  private static TransactionPayload payload(Map<String, JsonValue> metadata) throws Exception {
    byte[] seed = new byte[32];
    Arrays.fill(seed, (byte) 0x45);
    byte[] publicKey = new Ed25519PrivateKeyParameters(seed, 0).generatePublicKey().getEncoded();
    String authority = AccountAddress.fromAccount(publicKey, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    byte[] genesis = new byte[32];
    genesis[31] = 1;
    return new TransactionPayload(NetworkId.fromBytes(genesis), authority, 1_735_369_000_000L,
        Executable.ivm(new byte[] {1}), 100_000L, null,
        FeePaymentIntent.authority(Collections.emptyList(), 1L), TransactionAdmissionIntent.ORDINARY,
        metadata, null);
  }
}
