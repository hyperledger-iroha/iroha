package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandTypeV1;
import org.junit.Test;

public final class IrohaPeerNfcV1AdversarialTests {
  @Test
  public void javaFacadeRejectsNonCanonicalExtendedApduAliases() {
    final byte[] aliasedGetInfo =
        new byte[] {(byte) 0x80, 0x10, 0, 0, 0, 0, 0x62};
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNfcV1.decodeCommand(aliasedGetInfo));

    final byte[] session = repeat(16, 0x21);
    final byte[] hash = repeat(32, 0x31);
    final byte[] canonicalWrite =
        IrohaPeerNfcV1.encodeCommand(
            IrohaPeerNfcCommandV1.write(session, hash, 0, new byte[] {0x55}));
    final int bodyLength = canonicalWrite.length - 5;
    final byte[] aliasedWrite = new byte[7 + bodyLength];
    aliasedWrite[0] = (byte) 0x80;
    aliasedWrite[1] = 0x21;
    aliasedWrite[5] = 0;
    aliasedWrite[6] = (byte) bodyLength;
    System.arraycopy(canonicalWrite, 5, aliasedWrite, 7, bodyLength);
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNfcV1.decodeCommand(aliasedWrite));
  }

  @Test
  public void javaFacadePreservesUnsignedU32OffsetWithoutWrap() {
    final byte[] session = repeat(16, 0x41);
    final byte[] hash = repeat(32, 0x51);
    final IrohaPeerNfcCommandV1 command =
        IrohaPeerNfcCommandV1.readRequest(session, hash, 0xffff_ffffL, 1);
    final IrohaPeerNfcCommandV1 decoded =
        IrohaPeerNfcV1.decodeCommand(IrohaPeerNfcV1.encodeCommand(command));

    assertEquals(IrohaPeerNfcCommandTypeV1.READ_REQUEST, decoded.getType());
    assertEquals(0xffff_ffffL, decoded.getOffset());
    assertArrayEquals(session, decoded.getSessionId());
    assertArrayEquals(hash, decoded.getFirstHash());
  }

  private static byte[] repeat(final int count, final int value) {
    final byte[] bytes = new byte[count];
    java.util.Arrays.fill(bytes, (byte) value);
    return bytes;
  }
}
