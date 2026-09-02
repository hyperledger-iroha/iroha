// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Locale;
import java.util.Set;
import java.util.stream.Collectors;
import org.hyperledger.iroha.sdk.offline.KagemushaAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentRequestV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionStatementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionVoucherV1;
import org.hyperledger.iroha.sdk.offline.KagemushaTransferStatementV1;
import org.junit.Test;

/** Java-mirror checks for the sole current Kagemusha V1 wire surface. */
public final class KagemushaWireV1Tests {
  @Test
  public void exactUnpaddedBase64UrlRoundTrip() {
    final byte[] raw = {(byte) 0xfb, (byte) 0xff, 0x00, 0x01};
    final String text = KagemushaWireV1.encodeText(KagemushaWirePayloadKindV1.PAYMENT, raw);
    assertEquals("kgm1:-_8AAQ", text);
    assertArrayEquals(raw, KagemushaWireV1.decodeText(KagemushaWirePayloadKindV1.PAYMENT, text));
  }

  @Test
  public void oldOrNonCanonicalTextFailsClosed() {
    for (final String text :
        Arrays.asList(
            "oc" + "1:-_8AAQ",
            "KGM1:-_8AAQ",
            "kgm1:",
            "kgm1:-_8AAQ==",
            "kgm1:-_8A AQ",
            "kgm1:-_8AAQ\n",
            "kgm1:+/8AAQ",
            "kgm1:AB",
            "kgm1:A")) {
      assertThrows(
          IllegalArgumentException.class,
          () -> KagemushaWireV1.decodeText(KagemushaWirePayloadKindV1.PAYMENT, text));
    }
  }

  @Test
  public void perValueBoundsAreExact() {
    for (final KagemushaWirePayloadKindV1 kind : KagemushaWirePayloadKindV1.values()) {
      final byte[] exact = new byte[kind.maximumRawBytes()];
      Arrays.fill(exact, (byte) 0xa5);
      final String text = KagemushaWireV1.encodeText(kind, exact);
      assertEquals(kind.maximumTextBytes(), text.length());
      assertArrayEquals(exact, KagemushaWireV1.decodeText(kind, text));
      assertThrows(
          IllegalArgumentException.class,
          () -> KagemushaWireV1.encodeText(kind, new byte[kind.maximumRawBytes() + 1]));
      assertThrows(
          IllegalArgumentException.class,
          () -> KagemushaWireV1.decodeText(kind, text + "A"));
    }
  }

  @Test
  public void inventoryAndCompactBoundsExactlyMirrorKotlin() {
    assertEquals(6_528, KagemushaWireV1.MAXIMUM_PAIRED_PROOF_BYTES);
    assertEquals(9_211, KagemushaWireV1.MAXIMUM_SESSION_RAW_BYTES);
    assertEquals(12_288, KagemushaWireV1.MAXIMUM_SESSION_TEXT_BYTES);
    assertEquals(
        Arrays.asList(
            KagemushaWirePayloadKindV1.PAYMENT_REQUEST,
            KagemushaWirePayloadKindV1.PAYMENT,
            KagemushaWirePayloadKindV1.ACKNOWLEDGEMENT,
            KagemushaWirePayloadKindV1.MINT_AUTHORIZATION,
            KagemushaWirePayloadKindV1.MINT_CREDIT,
            KagemushaWirePayloadKindV1.REDEMPTION_VOUCHER),
        Arrays.asList(KagemushaWirePayloadKindV1.values()));
    assertEquals(
        Arrays.toString(org.hyperledger.iroha.sdk.offline.KagemushaWirePayloadKindV1.values()),
        Arrays.toString(KagemushaWirePayloadKindV1.values()));
  }

  @Test
  public void peerWireHasExactlyThreeMessageKinds() {
    assertEquals(
        new HashSet<>(Arrays.asList("RECEIVE_REQUEST", "PAYMENT", "ACKNOWLEDGEMENT")),
        Arrays.stream(IrohaPeerPayloadKind.values())
            .map(Enum::name)
            .collect(Collectors.toSet()));
  }

  @Test
  public void publicPeerRecordsContainNoRetiredNegotiationOrCertificateFields() {
    final Set<String> forbidden =
        new HashSet<>(
            Arrays.asList(
                "acceptanceintent",
                "acceptanceticket",
                "commitcertificate",
                "commitwrapper",
                "commitevidence",
                "outboxreservation",
                "artifactmanifest"));
    for (final Class<?> type :
        Arrays.asList(
            KagemushaPaymentRequestV1.class,
            KagemushaTransferStatementV1.class,
            KagemushaPaymentV1.class,
            KagemushaAcknowledgementV1.class,
            KagemushaRedemptionStatementV1.class,
            KagemushaRedemptionVoucherV1.class)) {
      final Set<String> names =
          Arrays.stream(type.getDeclaredFields())
              .map(field -> field.getName().toLowerCase(Locale.ROOT))
              .collect(Collectors.toSet());
      names.addAll(
          Arrays.stream(type.getDeclaredMethods())
              .map(Method::getName)
              .map(name -> name.toLowerCase(Locale.ROOT))
              .collect(Collectors.toSet()));
      for (final String fragment : forbidden) {
        assertFalse(
            type.getSimpleName() + " leaked " + fragment,
            names.stream().anyMatch(name -> name.contains(fragment)));
      }
    }
  }

  @Test
  public void requestPaymentAndRedemptionExposeCanonicalV1Bindings() {
    final Set<String> requestMembers = memberNames(KagemushaPaymentRequestV1.class);
    assertTrue(requestMembers.contains("recipientLaneId"));
    assertTrue(requestMembers.contains("recipientEncryptionKey"));

    final Set<String> transferMembers = memberNames(KagemushaTransferStatementV1.class);
    assertTrue(transferMembers.contains("senderBeforeCommitment"));
    assertTrue(transferMembers.contains("senderAfterCommitment"));
    assertTrue(transferMembers.contains("recipientLaneId"));
    assertTrue(transferMembers.contains("recipientEncryptionKey"));
    assertTrue(transferMembers.contains("committedAtMs"));
    assertTrue(transferMembers.contains("hardwareTransitionCommitment"));

    final Set<String> redemptionMembers = memberNames(KagemushaRedemptionStatementV1.class);
    assertTrue(redemptionMembers.contains("senderBeforeCommitment"));
    assertTrue(redemptionMembers.contains("senderAfterCommitment"));
    assertTrue(redemptionMembers.contains("committedAtMs"));
    assertTrue(redemptionMembers.contains("hardwareTransitionCommitment"));
  }

  private static Set<String> memberNames(final Class<?> type) {
    final Set<String> names =
        Arrays.stream(type.getDeclaredMethods()).map(Method::getName).collect(Collectors.toSet());
    names.addAll(
        Arrays.stream(type.getDeclaredFields()).map(field -> field.getName()).collect(Collectors.toSet()));
    return names;
  }
}
