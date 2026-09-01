package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertThrows;

import java.util.Arrays;
import org.junit.Test;

public final class KagemushaPeerTextBoundsTests {
  @Test
  public void oversizedTextFailsBeforeBase64Allocation() {
    final char[] characters =
        new char[KagemushaPeerTransport.MAXIMUM_TEXT_ENVELOPE_BYTES + 4];
    Arrays.fill(characters, 'A');
    final String oversized = new String(characters);

    assertThrows(IllegalArgumentException.class, () -> KagemushaPeerTransport.decode(oversized));
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaPeerTransport.decodeUserPresented(oversized, null));
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaPeerTransport.base64UrlDecode(oversized));
  }
}
