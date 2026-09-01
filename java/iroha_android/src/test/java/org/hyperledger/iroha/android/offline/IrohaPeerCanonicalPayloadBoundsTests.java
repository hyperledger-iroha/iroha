package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertThrows;

import org.junit.Test;

public final class IrohaPeerCanonicalPayloadBoundsTests {
  @Test
  public void oversizedCanonicalPayloadFailsBeforeDefensiveCopy() {
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                IrohaPeerPayloadKind.PAYMENT,
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND.requiredSchemaVersion(),
                new byte[IrohaPeerWireMessageV1.MAXIMUM_CANONICAL_BYTES + 1]));
  }
}
