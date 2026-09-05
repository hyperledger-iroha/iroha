// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandTypeV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandV1;
import org.junit.Test;

/** Adversarial checks for the canonical direct three-message NFC vocabulary. */
public final class IrohaPeerNfcV1AdversarialTests {
  @Test
  public void javaFacadeRoundTripsTheDirectApduVocabulary() {
    final IrohaPeerNfcCommandV1 request = IrohaPeerNfcCommandV1.readRequest(17, 23);
    final IrohaPeerNfcCommandV1 decodedRequest =
        IrohaPeerNfcV1.decodeCommand(IrohaPeerNfcV1.encodeCommand(request));
    assertEquals(IrohaPeerNfcCommandTypeV1.READ_REQUEST, decodedRequest.type);
    assertEquals(17, decodedRequest.offset);
    assertEquals(23, decodedRequest.length);

    final IrohaPeerNfcCommandV1 payment =
        IrohaPeerNfcCommandV1.writePayment(29, new byte[] {0x55, 0x66});
    final IrohaPeerNfcCommandV1 decodedPayment =
        IrohaPeerNfcV1.decodeCommand(IrohaPeerNfcV1.encodeCommand(payment));
    assertEquals(IrohaPeerNfcCommandTypeV1.WRITE_PAYMENT, decodedPayment.type);
    assertEquals(29, decodedPayment.offset);
    assertArrayEquals(new byte[] {0x55, 0x66}, decodedPayment.bytes());
  }

  @Test
  public void javaFacadeRejectsNonCanonicalExtendedApduAliases() {
    final byte[] aliasedGetInfo =
        new byte[] {(byte) 0x80, 0x10, 0, 0, 0, 0, 0x62};
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNfcV1.decodeCommand(aliasedGetInfo));
  }

  @Test
  public void noDataCommandHasOneCanonicalEncoding() {
    assertArrayEquals(
        new byte[] {(byte) 0x80, 0x10, 0, 0, 0, 0, 0},
        IrohaPeerNfcV1.encodeCommand(IrohaPeerNfcCommandV1.GET_INFO));
  }
}
