package org.hyperledger.iroha.android.model;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotEquals;
import static org.junit.Assert.fail;

import java.util.Arrays;
import java.util.Locale;
import org.hyperledger.iroha.android.util.HashLiteral;
import org.junit.Test;

/** Exact nominal-network identity tests. */
public final class NetworkIdTests {
  private static final String CANONICAL =
      "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";

  @Test
  public void networkIdRoundTripsLiteralAndRawBytes() {
    final NetworkId parsed = NetworkId.parse(CANONICAL);
    final NetworkId fromBytes = NetworkId.fromBytes(parsed.bytes());

    assertEquals(CANONICAL, parsed.literal());
    assertEquals(CANONICAL, parsed.toString());
    assertEquals(parsed, fromBytes);
    assertEquals(parsed.hashCode(), fromBytes.hashCode());
    assertArrayEquals(HashLiteral.decode(CANONICAL), parsed.bytes());
  }

  @Test
  public void networkIdRejectsNonCanonicalText() {
    for (final String value :
        Arrays.asList(
            CANONICAL.toLowerCase(Locale.ROOT),
            CANONICAL.substring(0, CANONICAL.length() - 4) + "0000",
            CANONICAL.substring(0, CANONICAL.indexOf('#')),
            CANONICAL.substring(5, CANONICAL.indexOf('#')),
            "network-label",
            " " + CANONICAL,
            CANONICAL + " ")) {
      expectIllegalArgument(() -> NetworkId.parse(value));
    }
    expectIllegalArgument(() -> NetworkId.parse(null));
  }

  @Test
  public void networkIdRequiresExactCanonicalRawBytes() {
    expectIllegalArgument(() -> NetworkId.fromBytes(null));
    expectIllegalArgument(() -> NetworkId.fromBytes(new byte[31]));
    expectIllegalArgument(() -> NetworkId.fromBytes(new byte[33]));

    final byte[] missingMarker = HashLiteral.decode(CANONICAL);
    missingMarker[missingMarker.length - 1] &= (byte) 0xFE;
    expectIllegalArgument(() -> NetworkId.fromBytes(missingMarker));
  }

  @Test
  public void networkIdDefensivelyCopiesRawBytes() {
    final byte[] source = HashLiteral.decode(CANONICAL);
    final NetworkId networkId = NetworkId.fromBytes(source);
    source[0] ^= 0x7F;
    final byte[] exposed = networkId.bytes();
    exposed[1] ^= 0x7F;

    assertEquals(CANONICAL, networkId.literal());
    assertNotEquals(source[0], networkId.bytes()[0]);
    assertNotEquals(exposed[1], networkId.bytes()[1]);
  }

  private static void expectIllegalArgument(final Runnable action) {
    try {
      action.run();
      fail("Expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected fail-closed rejection.
    }
  }
}
