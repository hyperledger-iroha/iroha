package org.hyperledger.iroha.sdk.consensus;

import static org.junit.jupiter.api.Assertions.*;
import org.junit.jupiter.api.Test;

/** Java-source consumers exercise the same Kotlin-owned canonical frame codec. */
public final class SumeragiStatusWireTests {
  private static byte[] bytes(String hex) {
    byte[] value = new byte[hex.length() / 2];
    for (int i = 0; i < value.length; i++) value[i] = (byte)Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16);
    return value;
  }
  @Test public void everyRustFrameMatchesJsonWithoutASecondJavaCodec() {
    assertEquals(8, NativeStatusFixtures.rows().size());
    NativeStatusFixtures.rows().forEach((name, row) -> {
      byte[] wire = bytes(row.getSecond());
      SumeragiStatus status = SumeragiStatusWire.decodeCanonical(wire);
      assertEquals(SumeragiStatus.parseJson(row.getFirst()), status, name);
      assertArrayEquals(wire, SumeragiStatusWire.encode(status), name);
    });
  }
  @Test public void truncationAndAlternateHeadersAreRejected() {
    byte[] wire = bytes(NativeStatusFixtures.rows().get("validator").getSecond());
    for (int offset : new int[]{0, 4, 5, 6, 22, 23, 31, 39}) {
      byte[] changed = wire.clone(); changed[offset] ^= 1;
      assertThrows(RuntimeException.class, () -> SumeragiStatusWire.decodeCanonical(changed));
    }
    assertThrows(RuntimeException.class, () -> SumeragiStatusWire.decodeCanonical(new byte[1_048_577]));
    assertThrows(RuntimeException.class, () -> SumeragiStatusWire.decodeCanonical(new byte[0]));
  }
}
