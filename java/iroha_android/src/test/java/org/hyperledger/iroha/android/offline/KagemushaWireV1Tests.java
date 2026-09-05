// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.hyperledger.iroha.sdk.offline.KagemushaAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentRequestV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentV1;
import org.junit.Test;

public final class KagemushaWireV1Tests {
  @Test
  public void v1PeerExchangeHasOnlyRequestPaymentAndAcknowledgement() {
    assertArrayEquals(
        new IrohaPeerPayloadKind[] {
          IrohaPeerPayloadKind.REQUEST,
          IrohaPeerPayloadKind.PAYMENT,
          IrohaPeerPayloadKind.ACKNOWLEDGEMENT
        },
        IrohaPeerPayloadKind.values());
    assertEquals(1, IrohaPeerPayloadKind.REQUEST.code());
    assertEquals(2, IrohaPeerPayloadKind.PAYMENT.code());
    assertEquals(3, IrohaPeerPayloadKind.ACKNOWLEDGEMENT.code());
  }

  @Test
  public void v1UsesTheKagemushaTextPrefix() {
    assertEquals("kgm1:", KagemushaWireV1.TEXT_PREFIX);
  }

  @Test
  public void sharedFixtureRoundTripsOnlyTheThreeMessageProtocol() throws Exception {
    final String fixture = loadFixture();
    assertEquals(1L, fixtureLong(fixture, "fixture_version"));
    assertFalse(fixture.contains("\"acceptance_intent\""));
    assertFalse(fixture.contains("\"acceptance_ticket\""));
    assertFalse(fixture.contains("\"complete_five_message\""));

    final byte[] requestBytes = fixtureBytes(fixture, "payment_request");
    final byte[] paymentBytes = fixtureBytes(fixture, "payment");
    final byte[] acknowledgementBytes = fixtureBytes(fixture, "acknowledgement");
    final KagemushaPaymentRequestV1 request =
        KagemushaNoritoV1.decodePaymentRequestShapeExact(requestBytes);
    final KagemushaPaymentV1 payment =
        KagemushaNoritoV1.decodePaymentShapeExact(paymentBytes, request);
    final KagemushaAcknowledgementV1 acknowledgement =
        KagemushaNoritoV1.decodeAcknowledgementShapeExact(
            acknowledgementBytes, request, payment);

    assertArrayEquals(requestBytes, KagemushaNoritoV1.encodePaymentRequestShape(request));
    assertArrayEquals(paymentBytes, KagemushaNoritoV1.encodePaymentShape(payment, request));
    assertArrayEquals(
        acknowledgementBytes,
        KagemushaNoritoV1.encodeAcknowledgementShape(acknowledgement, request, payment));
  }

  private static String loadFixture() throws Exception {
    Path current = Paths.get("").toAbsolutePath().normalize();
    while (current != null) {
      final Path candidate = current.resolve("fixtures/offline/kagemusha_v1.json");
      if (Files.isRegularFile(candidate)) {
        return new String(Files.readAllBytes(candidate), StandardCharsets.UTF_8);
      }
      current = current.getParent();
    }
    throw new AssertionError("fixtures/offline/kagemusha_v1.json was not found");
  }

  private static byte[] fixtureBytes(final String fixture, final String section) {
    final Matcher sectionMatcher =
        Pattern.compile(
                "\\\"" + Pattern.quote(section)
                    + "\\\"\\s*:\\s*\\{.*?\\\"norito_hex\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"",
                Pattern.DOTALL)
            .matcher(fixture);
    if (!sectionMatcher.find()) {
      throw new AssertionError("fixture section " + section + " was not found");
    }
    final String hex = sectionMatcher.group(1);
    if ((hex.length() & 1) != 0) {
      throw new AssertionError("fixture hex length is odd");
    }
    final byte[] bytes = new byte[hex.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      final int high = Character.digit(hex.charAt(index * 2), 16);
      final int low = Character.digit(hex.charAt(index * 2 + 1), 16);
      if (high < 0 || low < 0) {
        throw new AssertionError("fixture hex is invalid");
      }
      bytes[index] = (byte) ((high << 4) | low);
    }
    return bytes;
  }

  private static long fixtureLong(final String fixture, final String field) {
    final Matcher matcher =
        Pattern.compile("\\\"" + Pattern.quote(field) + "\\\"\\s*:\\s*(\\d+)")
            .matcher(fixture);
    if (!matcher.find()) {
      throw new AssertionError("fixture field " + field + " was not found");
    }
    return Long.parseLong(matcher.group(1));
  }
}
