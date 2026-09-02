// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.io.File;
import java.io.IOException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Arrays;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.hyperledger.iroha.sdk.offline.OfflineCashAcceptanceIntentAuthorizationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAcceptanceTicketV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAssetDefinitionIdV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAssetIncarnationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashCommitCertificateV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashCommitEvidenceV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashCreditOpeningV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashEncryptedCreditAadV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashEncryptedCreditEnvelopeV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashEncryptedCreditPurposeV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashMintAuthorizationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashMintCreditV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashNoCommitClosureStatementV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashNoCommitClosureV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashOperationKindV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashOutboxReservationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashPastaStateCommitmentV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashPaymentRequestV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashPaymentV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashRedemptionVoucherV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashX25519PublicKeyV1;
import org.junit.Test;

/** Java-mirror checks for the sole current Offline Cash V1 wire surface. */
public final class OfflineCashWireV1Tests {
  @Test
  public void exactUnpaddedBase64UrlRoundTrip() {
    final byte[] raw = {(byte) 0xfb, (byte) 0xff, 0x00, 0x01};
    final String text = OfflineCashWireV1.encodeText(OfflineCashWirePayloadKindV1.PAYMENT, raw);
    assertEquals("oc1:-_8AAQ", text);
    assertArrayEquals(raw, OfflineCashWireV1.decodeText(OfflineCashWirePayloadKindV1.PAYMENT, text));
  }

  @Test
  public void nonCanonicalTextFailsClosed() {
    for (final String text :
        Arrays.asList(
            "OC1:-_8AAQ",
            "oc1:",
            "oc1:-_8AAQ==",
            "oc1:-_8A AQ",
            "oc1:-_8AAQ\n",
            "oc1:+/8AAQ",
            "oc1:AB",
            "oc1:A")) {
      assertThrows(
          IllegalArgumentException.class,
          () -> OfflineCashWireV1.decodeText(OfflineCashWirePayloadKindV1.PAYMENT, text));
    }
  }

  @Test
  public void perValueBoundsAreExact() {
    for (final OfflineCashWirePayloadKindV1 kind : OfflineCashWirePayloadKindV1.values()) {
      final byte[] exact = new byte[kind.maximumRawBytes()];
      Arrays.fill(exact, (byte) 0xa5);
      final String text = OfflineCashWireV1.encodeText(kind, exact);
      assertEquals(kind.maximumTextBytes(), text.length());
      assertArrayEquals(exact, OfflineCashWireV1.decodeText(kind, text));
      assertThrows(
          IllegalArgumentException.class,
          () -> OfflineCashWireV1.encodeText(kind, new byte[kind.maximumRawBytes() + 1]));
      assertThrows(
          IllegalArgumentException.class,
          () -> OfflineCashWireV1.decodeText(kind, text + "A"));
    }
  }

  @Test
  public void currentCapsAndInventoryExactlyMirrorKotlin() {
    assertEquals(1_024, OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES);
    assertEquals(1_370, OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_TEXT_BYTES);
    assertEquals(9_984, OfflineCashWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_RAW_BYTES);
    assertEquals(13_326, OfflineCashWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_TEXT_BYTES);
    assertEquals(18_171, OfflineCashWireV1.MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES);
    assertEquals(24_244, OfflineCashWireV1.MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES);
    assertEquals(200, OfflineCashWireV1.CREDIT_OPENING_CANONICAL_BYTES);
    assertEquals(216, OfflineCashWireV1.ENCRYPTED_CREDIT_CIPHERTEXT_AND_TAG_BYTES);
    assertEquals(16_384, OfflineCashWireV1.MAXIMUM_NO_COMMIT_CLOSURE_BYTES);
    assertEquals(9, OfflineCashWirePayloadKindV1.values().length);
    assertEquals(
        Arrays.toString(org.hyperledger.iroha.sdk.offline.OfflineCashWirePayloadKindV1.values()),
        Arrays.toString(OfflineCashWirePayloadKindV1.values()));
    assertEquals(
        org.hyperledger.iroha.sdk.offline.OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES,
        OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES);
  }

  @Test
  public void outboxReservationUsesTheFixedCircuitTranscript() {
    final OfflineCashOutboxReservationV1 reservation =
        new OfflineCashOutboxReservationV1(
            fill(32, (byte) 0xd1),
            OfflineCashOperationKindV1.SEND_SPLIT,
            OfflineCashWireV1.PAYMENT_OUTBOX_MIN_BYTES,
            7L,
            11L);
    assertArrayEquals(
        hexBytes("37fc277d5644739bdf04a6b3828246c1158df9db936bf9d8189aa19ea5eb6fdb"),
        OfflineCashNoritoV1.outboxReservationCommitmentShape(reservation));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            OfflineCashNoritoV1.outboxReservationCommitmentShape(
                new OfflineCashOutboxReservationV1(
                    fill(32, (byte) 0xd2),
                    OfflineCashOperationKindV1.SEND_SPLIT,
                    OfflineCashWireV1.PAYMENT_OUTBOX_MIN_BYTES - 1,
                    7L,
                    11L)));
  }

  @Test
  public void signedJvmCarriersPreserveTheFullRustUnsignedTranscriptDomain() {
    final OfflineCashOutboxReservationV1 boundaryReservation =
        new OfflineCashOutboxReservationV1(
            fill(32, (byte) 0xd1),
            OfflineCashOperationKindV1.SEND_SPLIT,
            -1,
            Long.MAX_VALUE,
            Long.MIN_VALUE);
    assertArrayEquals(
        hexBytes("fc904c99266ca1728181789f606b6e421b90a04fe99edb1c8bc236f73b063b0e"),
        OfflineCashNoritoV1.outboxReservationCommitmentShape(boundaryReservation));

    final OfflineCashCommitCertificateV1 boundaryCertificate =
        new OfflineCashCommitCertificateV1(
            1,
            fill(32, (byte) 0xc1),
            fill(32, (byte) 0xc2),
            fill(32, (byte) 0xc3),
            fill(32, (byte) 0xc4),
            fill(32, (byte) 0xc5),
            new OfflineCashCommitEvidenceV1.TrustedTime(fill(32, (byte) 0xc6)),
            fill(32, (byte) 0xc7),
            Long.MIN_VALUE,
            fill(32, (byte) 0xc8));
    assertArrayEquals(
        hexBytes("b1fe2841e59c24eda16d2509e124bcf786199e9238cb5c168d0559aebe32cdc3"),
        OfflineCashNoritoV1.expectedCommitCertificateIdShape(boundaryCertificate));
    assertArrayEquals(
        hexBytes("d8f7d13446aa7c0704894c4563c93cb468829d9e17aa7e17dcb70eb939ecc275"),
        OfflineCashNoritoV1.commitCertificateDigestShape(boundaryCertificate));
  }

  @Test
  public void noCommitClosureFacadeHasNoPublicStateHeadsAndConsumesGeneratedFixtureWhenPresent()
      throws IOException {
    for (final java.lang.reflect.Field field :
        OfflineCashNoCommitClosureStatementV1.class.getDeclaredFields()) {
      final String name = field.getName().toLowerCase(java.util.Locale.ROOT);
      assertFalse(name.contains("predecessor"));
      assertFalse(name.contains("successor"));
      assertFalse(name.contains("statecommitment"));
      assertFalse(name.contains("beforesequence"));
      assertFalse(name.contains("aftersequence"));
    }
    assertEquals(
        OfflineCashWireV1.MAXIMUM_NO_COMMIT_CLOSURE_BYTES,
        org.hyperledger.iroha.sdk.offline.OfflineCashWireV1.MAXIMUM_NO_COMMIT_CLOSURE_BYTES);

    final String fixture = sharedFixtureText();
    final int sectionOffset = fixture.indexOf("\"no_commit_closure\"");
    if (sectionOffset < 0) {
      return;
    }
    final Matcher matcher =
        Pattern.compile("\\\"norito_hex\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"")
            .matcher(fixture.substring(sectionOffset));
    assertTrue(matcher.find());
    final byte[] raw = hexBytes(matcher.group(1));
    final OfflineCashNoCommitClosureV1 closure =
        OfflineCashNoritoV1.decodeNoCommitClosureShapeExact(raw);
    assertArrayEquals(raw, OfflineCashNoritoV1.encodeNoCommitClosureShape(closure));
    assertEquals(32, OfflineCashNoritoV1.noCommitClosureDigestShape(closure).length);
  }

  @Test
  public void nativeGeneratedV1FixtureRoundTripsEveryTransportedValue() throws IOException {
    final String fixture = sharedFixtureText();
    assertTrue(Pattern.compile("\\\"fixture_version\\\"\\s*:\\s*1").matcher(fixture).find());

    final byte[] requestRaw = fixtureBytes(fixture, "payment_request");
    final OfflineCashPaymentRequestV1 request =
        OfflineCashNoritoV1.decodePaymentRequestShapeExact(requestRaw);
    final byte[] authorizationRaw = fixtureBytes(fixture, "acceptance_intent_authorization");
    final OfflineCashAcceptanceIntentAuthorizationV1 authorization =
        OfflineCashNoritoV1.decodeAcceptanceIntentAuthorizationShapeExact(
            authorizationRaw, request);
    final byte[] ticketRaw = fixtureBytes(fixture, "acceptance_ticket");
    final OfflineCashAcceptanceTicketV1 ticket =
        OfflineCashNoritoV1.decodeAcceptanceTicketShapeExact(ticketRaw, request, authorization);
    final byte[] closureRaw = fixtureBytes(fixture, "no_commit_closure");
    final OfflineCashNoCommitClosureV1 closure =
        OfflineCashNoritoV1.decodeNoCommitClosureShapeExact(closureRaw);
    final byte[] paymentRaw = fixtureBytes(fixture, "payment");
    final OfflineCashPaymentV1 payment =
        OfflineCashNoritoV1.decodePaymentShapeExact(paymentRaw, request);
    final byte[] acknowledgementRaw = fixtureBytes(fixture, "acknowledgement");
    final OfflineCashAcknowledgementV1 acknowledgement =
        OfflineCashNoritoV1.decodeAcknowledgementShapeExact(
            acknowledgementRaw, request, payment);
    final byte[] mintAuthorizationRaw = fixtureBytes(fixture, "mint_authorization");
    final OfflineCashMintAuthorizationV1 mintAuthorization =
        OfflineCashNoritoV1.decodeMintAuthorizationShapeExact(mintAuthorizationRaw);
    final byte[] mintCreditRaw = fixtureBytes(fixture, "mint_credit");
    final OfflineCashMintCreditV1 mintCredit =
        OfflineCashNoritoV1.decodeMintCreditShapeExact(mintCreditRaw, mintAuthorization);
    final byte[] redemptionRaw = fixtureBytes(fixture, "redemption_voucher");
    final OfflineCashRedemptionVoucherV1 redemption =
        OfflineCashNoritoV1.decodeRedemptionVoucherShapeExact(redemptionRaw);
    final byte[] envelopeRaw = fixtureBytes(fixture, "encrypted_credit_envelope");
    final OfflineCashEncryptedCreditEnvelopeV1 envelope =
        OfflineCashNoritoV1.decodeEncryptedCreditEnvelopeShapeExact(envelopeRaw);
    final byte[] aadRaw = fixtureBytes(fixture, "encrypted_credit_aad");
    final OfflineCashEncryptedCreditAadV1 aad =
        OfflineCashNoritoV1.decodeEncryptedCreditAadShapeExact(aadRaw);
    final byte[] openingRaw = fixtureBytes(fixture, "credit_opening");
    final OfflineCashCreditOpeningV1 opening =
        OfflineCashNoritoV1.decodeCreditOpeningShapeExact(openingRaw);

    final List<byte[]> expected =
        Arrays.asList(
            requestRaw,
            authorizationRaw,
            ticketRaw,
            closureRaw,
            paymentRaw,
            acknowledgementRaw,
            mintAuthorizationRaw,
            mintCreditRaw,
            redemptionRaw,
            envelopeRaw,
            aadRaw,
            openingRaw);
    final List<byte[]> actual =
        Arrays.asList(
            OfflineCashNoritoV1.encodePaymentRequestShape(request),
            OfflineCashNoritoV1.encodeAcceptanceIntentAuthorizationShape(authorization, request),
            OfflineCashNoritoV1.encodeAcceptanceTicketShape(ticket, request, authorization),
            OfflineCashNoritoV1.encodeNoCommitClosureShape(closure),
            OfflineCashNoritoV1.encodePaymentShape(payment, request),
            OfflineCashNoritoV1.encodeAcknowledgementShape(
                acknowledgement, request, payment),
            OfflineCashNoritoV1.encodeMintAuthorizationShape(mintAuthorization),
            OfflineCashNoritoV1.encodeMintCreditShape(mintCredit, mintAuthorization),
            OfflineCashNoritoV1.encodeRedemptionVoucherShape(redemption),
            OfflineCashNoritoV1.encodeEncryptedCreditEnvelopeShape(envelope),
            OfflineCashNoritoV1.encodeEncryptedCreditAadShape(aad),
            OfflineCashNoritoV1.encodeCreditOpeningShape(opening));

    assertEquals(expected.size(), actual.size());
    for (int index = 0; index < expected.size(); index++) {
      assertArrayEquals("fixture value " + index, expected.get(index), actual.get(index));
    }
  }

  @Test
  public void typedOpeningAadEnvelopeAndX25519ShapeChecksStayCodecOnly() {
    final byte[] creditId = fill(32, (byte) 3);
    final OfflineCashCreditOpeningV1 opening =
        new OfflineCashCreditOpeningV1(
            1,
            creditId,
            BigInteger.valueOf(19),
            fill(32, (byte) 4),
            fill(32, (byte) 5),
            fill(32, (byte) 6));
    final byte[] openingBytes = OfflineCashNoritoV1.encodeCreditOpeningShape(opening);
    assertEquals(OfflineCashWireV1.CREDIT_OPENING_CANONICAL_BYTES, openingBytes.length);
    assertEquals(
        BigInteger.valueOf(19),
        OfflineCashNoritoV1
            .decodeCreditOpeningShapeExactAgainst(openingBytes, creditId, BigInteger.valueOf(19))
            .amount);

    final OfflineCashEncryptedCreditAadV1 aad =
        new OfflineCashEncryptedCreditAadV1(
            1,
            OfflineCashEncryptedCreditPurposeV1.PEER,
            fill(32, (byte) 7),
            fill(32, (byte) 8),
            creditId,
            BigInteger.valueOf(19));
    final byte[] aadBytes = OfflineCashNoritoV1.encodeEncryptedCreditAadShape(aad);
    assertArrayEquals(
        aadBytes,
        OfflineCashNoritoV1.encodeEncryptedCreditAadShape(
            OfflineCashNoritoV1.decodeEncryptedCreditAadShapeExact(aadBytes)));

    final byte[] x25519 = new byte[32];
    x25519[0] = 9;
    final OfflineCashEncryptedCreditEnvelopeV1 envelope =
        new OfflineCashEncryptedCreditEnvelopeV1(
            1,
            new OfflineCashX25519PublicKeyV1(x25519),
            fill(24, (byte) 9),
            fill(216, (byte) 10));
    final byte[] envelopeBytes = OfflineCashNoritoV1.encodeEncryptedCreditEnvelopeShape(envelope);
    assertArrayEquals(
        envelopeBytes,
        OfflineCashNoritoV1.encodeEncryptedCreditEnvelopeShape(
            OfflineCashNoritoV1.decodeEncryptedCreditEnvelopeShapeExact(envelopeBytes)));
    assertThrows(
        IllegalArgumentException.class,
        () -> new OfflineCashX25519PublicKeyV1(new byte[32]));
    assertThrows(
        IllegalArgumentException.class,
        () -> new OfflineCashX25519PublicKeyV1(fill(31, (byte) 1)));
    assertThrows(
        IllegalArgumentException.class,
        () -> new OfflineCashX25519PublicKeyV1(fill(33, (byte) 1)));
    final byte[] nonzeroLowOrderWireShape = new byte[32];
    nonzeroLowOrderWireShape[0] = 1;
    assertArrayEquals(
        "managed codecs must not probe X25519 elements or provide a software-crypto fallback",
        nonzeroLowOrderWireShape,
        new OfflineCashX25519PublicKeyV1(nonzeroLowOrderWireShape).bytes());
  }

  @Test
  public void javaFacadeOnlyExposesShapeNamedCodecOperations() {
    for (final Method method : OfflineCashNoritoV1.class.getDeclaredMethods()) {
      if (!Modifier.isPublic(method.getModifiers()) || !Modifier.isStatic(method.getModifiers())) {
        continue;
      }
      final String name = method.getName();
      if (name.startsWith("encode") || name.startsWith("decode") || name.startsWith("validate")) {
        assertFalse("non-shape codec name: " + name, name.endsWith("PaymentRequest"));
        if (name.startsWith("decode")) {
          assertTrue("retired exact-less decoder: " + name, name.contains("ShapeExact"));
        }
      }
    }
  }

  @Test
  public void typedJavaFacadeUsesTheExactKotlinCanonicalImplementation() {
    final byte[] networkBytes = new byte[32];
    networkBytes[31] = 1;
    final NetworkId network = NetworkId.fromBytes(networkBytes);
    final OfflineCashAssetDefinitionIdV1 asset =
        OfflineCashAssetDefinitionIdV1.parse("6TEAJqbb8oEPmLncoNiMRbLEK6tw");
    final OfflineCashAssetIncarnationV1 incarnation =
        new OfflineCashAssetIncarnationV1(fill(32, (byte) 11));
    assertArrayEquals(
        org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.liabilityPoolId(
            network, asset, incarnation),
        OfflineCashNoritoV1.liabilityPoolId(network, asset, incarnation));
    final OfflineCashPastaStateCommitmentV1 state =
        new OfflineCashPastaStateCommitmentV1(fill(32, (byte) 5), fill(32, (byte) 7));
    assertArrayEquals(
        org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.pastaStateCommitment(state),
        OfflineCashNoritoV1.pastaStateCommitment(state));
  }

  private static byte[] fill(final int size, final byte value) {
    final byte[] result = new byte[size];
    Arrays.fill(result, value);
    return result;
  }

  private static String sharedFixtureText() throws IOException {
    File directory = new File(System.getProperty("user.dir"));
    while (directory != null) {
      final File candidate = new File(directory, "fixtures/offline/offline_cash_v1.json");
      if (candidate.isFile()) {
        return new String(Files.readAllBytes(candidate.toPath()), StandardCharsets.UTF_8);
      }
      directory = directory.getParentFile();
    }
    throw new IOException("fixtures/offline/offline_cash_v1.json not found");
  }

  private static byte[] fixtureBytes(final String fixture, final String section) {
    final int sectionOffset = fixture.indexOf("\"" + section + "\"");
    if (sectionOffset < 0) {
      throw new IllegalArgumentException("missing fixture section " + section);
    }
    final Matcher matcher =
        Pattern.compile("\\\"norito_hex\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"")
            .matcher(fixture.substring(sectionOffset));
    if (!matcher.find()) {
      throw new IllegalArgumentException("missing norito_hex for fixture section " + section);
    }
    return hexBytes(matcher.group(1));
  }

  private static byte[] hexBytes(final String value) {
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      result[index] =
          (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return result;
  }
}
