package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertThrows;

import org.junit.Test;

public final class IrohaPeerNearbyAndroidBoundsTests {
  @Test
  public void discoveryFacadeRejectsOversizedInputsBeforeCopying() {
    assertThrows(
        IllegalArgumentException.class,
        () ->
            IrohaPeerNearbyAndroidV1.discoveryContext(
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                IrohaPeerNearbyRoleV1.SENDER,
                new byte[17],
                filled(32, 2)));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            IrohaPeerNearbyAndroidV1.discoveryContext(
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                IrohaPeerNearbyRoleV1.SENDER,
                filled(16, 1),
                new byte[33]));
  }

  private static byte[] filled(final int count, final int value) {
    final byte[] bytes = new byte[count];
    java.util.Arrays.fill(bytes, (byte) value);
    return bytes;
  }
}
