// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.lang.reflect.Method;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.function.Consumer;
import java.util.stream.Collectors;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.hyperledger.iroha.sdk.offline.KagemushaAcceptanceIntentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAcceptanceTicketV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceMintStageCommandV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceMintStageResultV1;
import org.hyperledger.iroha.sdk.offline.KagemushaLifecycleBindingV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintAuthorizationContextV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintAuthorizationStatementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintAuthorizationV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintCreditStatementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintCreditV1;
import org.hyperledger.iroha.sdk.offline.KagemushaOperationKindV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPairedProofV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentRequestV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentOutputV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionProofV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionStatementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionVoucherV1;
import org.hyperledger.iroha.sdk.offline.KagemushaX25519PublicKeyV1;
import org.hyperledger.iroha.sdk.norito.NoritoHeader;
import org.junit.Test;

/** Java-mirror checks for the sole current KAGEMUSHA V1 wire surface. */
public final class KagemushaWireV1Tests {
  @Test
  public void sharedCanonicalFixtureRoundTripsByteForByte() throws Exception {
    final String fixture = loadCanonicalFixture();
    assertEquals(1L, fixtureLong(fixture, null, "fixture_version"));
    assertEquals("KAGEMUSHA", fixtureString(fixture, null, "protocol"));
    assertEquals("kgm1:", fixtureString(fixture, null, "text_prefix"));
    assertEquals(
        Arrays.asList(
            "request:1", "intent:2", "ticket:3", "payment:4", "acknowledgement:5"),
        fixtureMessageOrder(fixture));

    final byte[] requestRaw = fixtureHex(fixture, "payment_request");
    final byte[] intentRaw = fixtureHex(fixture, "acceptance_intent");
    final byte[] ticketRaw = fixtureHex(fixture, "acceptance_ticket");
    final byte[] paymentRaw = fixtureHex(fixture, "payment");
    final byte[] acknowledgementRaw = fixtureHex(fixture, "acknowledgement");
    final String[] sections = {
      "payment_request",
      "acceptance_intent",
      "acceptance_ticket",
      "payment",
      "acknowledgement"
    };
    final byte[][] rawSections = {
      requestRaw, intentRaw, ticketRaw, paymentRaw, acknowledgementRaw
    };
    final KagemushaWirePayloadKindV1[] textKinds = {
      KagemushaWirePayloadKindV1.PAYMENT_REQUEST,
      KagemushaWirePayloadKindV1.ACCEPTANCE_INTENT,
      KagemushaWirePayloadKindV1.ACCEPTANCE_TICKET,
      KagemushaWirePayloadKindV1.PAYMENT,
      KagemushaWirePayloadKindV1.ACKNOWLEDGEMENT
    };
    for (int index = 0; index < sections.length; index++) {
      assertEquals(index + 1L, fixtureLong(fixture, sections[index], "ipm1_kind"));
      assertEquals(rawSections[index].length, fixtureLong(fixture, sections[index], "raw_bytes"));
      assertEquals(
          fixtureString(fixture, sections[index], "sha256"), sha256Hex(rawSections[index]));
      final String text = fixtureString(fixture, sections[index], "kgm1");
      assertArrayEquals(rawSections[index], KagemushaWireV1.decodeText(textKinds[index], text));
      assertEquals(text, KagemushaWireV1.encodeText(textKinds[index], rawSections[index]));
    }
    final KagemushaPaymentRequestV1 request =
        KagemushaNoritoV1.decodePaymentRequestShapeExact(requestRaw);
    final KagemushaAcceptanceIntentV1 intent =
        KagemushaNoritoV1.decodeAcceptanceIntentShapeExact(intentRaw, request);
    final KagemushaAcceptanceTicketV1 ticket =
        KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
            ticketRaw, request, intent);
    final KagemushaPaymentV1 payment =
        KagemushaNoritoV1.decodePaymentShapeExact(paymentRaw, request, intent, ticket);
    final KagemushaAcknowledgementV1 acknowledgement =
        KagemushaNoritoV1.decodeAcknowledgementShapeExact(
            acknowledgementRaw, request, payment, intent, ticket);
    final byte[] paymentProofRaw = KagemushaNoritoV1.encodePaymentProofShape(payment.proof);
    assertArrayEquals(
        org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1.encodePaymentProofShape(payment.proof),
        paymentProofRaw);
    assertArrayEquals(
        paymentProofRaw,
        KagemushaNoritoV1.encodePaymentProofShape(
            KagemushaNoritoV1.decodePaymentProofShapeExact(paymentProofRaw)));
    assertTrue(paymentProofRaw.length <= KagemushaWireV1.MAXIMUM_PAIRED_PROOF_BYTES);
    assertArrayEquals(
        org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1.paymentOutputDigestShape(payment.output),
        KagemushaNoritoV1.paymentOutputDigestShape(payment.output));
    assertArrayEquals(
        org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1.paymentBodyDigestShape(
            payment.output, payment.encryptedCredit()),
        KagemushaNoritoV1.paymentBodyDigestShape(payment.output, payment.encryptedCredit()));
    assertArrayEquals(
        payment.proof.semanticDigest(),
        KagemushaNoritoV1.paymentBodyDigestShape(payment.output, payment.encryptedCredit()));
    assertArrayEquals(
        org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1.assetIdentityCanonicalShape(request.asset),
        KagemushaNoritoV1.assetIdentityCanonicalShape(request.asset));
    assertArrayEquals(
        org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1.accountIdentityCanonicalShape(request.recipient),
        KagemushaNoritoV1.accountIdentityCanonicalShape(request.recipient));
    assertEquals(
        fixtureString(fixture, "identity_vectors", "acceptance_intent_digest_hex"),
        hex(KagemushaNoritoV1.acceptanceIntentDigestShape(intent, request)));
    assertArrayEquals(
        fixtureHexValue(fixture, "identity_vectors", "transition_nullifier_hex"),
        payment.output.transitionNullifier());
    assertArrayEquals(
        fixtureHexValue(fixture, "identity_vectors", "prepared_transfer_digest_hex"),
        KagemushaNoritoV1.preparedTransferDigestShape(
            request, intent, ticket, payment.output.transitionNullifier(),
            payment.output.ciphertextCommitment()));
    assertEquals(
        fixtureString(fixture, "identity_vectors", "credit_id_hex"),
        hex(payment.output.creditId()));
    assertArrayEquals(
        fixtureHexValue(fixture, "identity_vectors", "credit_id_hex"),
        KagemushaNoritoV1.expectedPeerCreditIdShape(payment.output, request, intent));
    assertEquals(
        fixtureString(fixture, "peer_credit_opening", "commitment_hex"),
        hex(
            KagemushaNoritoV1.peerCreditOpeningCommitmentShape(
                fixtureHexValue(fixture, "peer_credit_opening", "request_digest_hex"),
                new KagemushaX25519PublicKeyV1(
                    fixtureHexValue(
                        fixture, "peer_credit_opening", "recipient_one_time_key_hex")),
                BigInteger.valueOf(fixtureLong(fixture, "peer_credit_opening", "amount")),
                fixtureHexValue(
                    fixture, "peer_credit_opening", "credit_commitment_opening_hex"),
                fixtureHexValue(
                    fixture, "peer_credit_opening", "recipient_binding_opening_hex"),
                fixtureHexValue(fixture, "peer_credit_opening", "recovery_nonce_hex"))));

    assertArrayEquals(requestRaw, KagemushaNoritoV1.encodePaymentRequestShape(request));
    assertArrayEquals(
        intentRaw,
        KagemushaNoritoV1.encodeAcceptanceIntentShape(intent, request));
    assertArrayEquals(
        ticketRaw, KagemushaNoritoV1.encodeAcceptanceTicketShape(ticket, request, intent));
    assertArrayEquals(
        paymentRaw,
        KagemushaNoritoV1.encodePaymentShape(payment, request, intent, ticket));
    assertArrayEquals(
        acknowledgementRaw,
        KagemushaNoritoV1.encodeAcknowledgementShape(
            acknowledgement, request, payment, intent, ticket));
    assertEquals(
        requestRaw.length
            + intentRaw.length
            + ticketRaw.length
            + paymentRaw.length
            + acknowledgementRaw.length,
        KagemushaNoritoV1.validateCompleteExchangeShape(
            request, intent, ticket, payment, acknowledgement));
    assertEquals(
        requestRaw.length
            + intentRaw.length
            + ticketRaw.length
            + paymentRaw.length
            + acknowledgementRaw.length,
        fixtureLong(fixture, "complete_five_message", "raw_bytes"));

    assertPeerRoundTrip(IrohaPeerPayloadKind.REQUEST, requestRaw);
    assertPeerRoundTrip(IrohaPeerPayloadKind.INTENT, intentRaw);
    assertPeerRoundTrip(IrohaPeerPayloadKind.TICKET, ticketRaw);
    assertPeerRoundTrip(IrohaPeerPayloadKind.PAYMENT, paymentRaw);
    assertPeerRoundTrip(IrohaPeerPayloadKind.ACKNOWLEDGEMENT, acknowledgementRaw);
  }

  @Test
  public void exactUnpaddedBase64UrlRoundTrip() {
    final byte[] raw = {(byte) 0xfb, (byte) 0xff, 0x00, 0x01};
    final String text = KagemushaWireV1.encodeText(KagemushaWirePayloadKindV1.PAYMENT, raw);
    assertEquals("kgm1:-_8AAQ", text);
    assertArrayEquals(raw, KagemushaWireV1.decodeText(KagemushaWirePayloadKindV1.PAYMENT, text));
    assertEquals(
        org.hyperledger.iroha.sdk.offline.KagemushaWireV1.encodeText(
            KagemushaWirePayloadKindV1.PAYMENT.canonicalKind(), raw),
        text);
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
      final IllegalArgumentException canonicalFailure = assertThrows(
          IllegalArgumentException.class,
          () -> org.hyperledger.iroha.sdk.offline.KagemushaWireV1.decodeText(
              KagemushaWirePayloadKindV1.PAYMENT.canonicalKind(), text));
      final IllegalArgumentException javaFailure = assertThrows(
          IllegalArgumentException.class,
          () -> KagemushaWireV1.decodeText(KagemushaWirePayloadKindV1.PAYMENT, text));
      assertEquals(canonicalFailure.getMessage(), javaFailure.getMessage());
    }
  }

  @Test
  public void perValueBoundsAreExact() {
    for (final KagemushaWirePayloadKindV1 kind : KagemushaWirePayloadKindV1.values()) {
      final org.hyperledger.iroha.sdk.offline.KagemushaWirePayloadKindV1 canonicalKind =
          kind.canonicalKind();
      assertEquals(kind.name(), canonicalKind.name());
      assertEquals(canonicalKind.getMaximumRawBytes(), kind.maximumRawBytes());
      assertEquals(canonicalKind.getMaximumTextBytes(), kind.maximumTextBytes());
      final byte[] exact = new byte[kind.maximumRawBytes()];
      Arrays.fill(exact, (byte) 0xa5);
      final String text = KagemushaWireV1.encodeText(kind, exact);
      assertEquals(kind.maximumTextBytes(), text.length());
      assertEquals(
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.encodeText(canonicalKind, exact),
          text);
      assertArrayEquals(exact, KagemushaWireV1.decodeText(kind, text));
      assertArrayEquals(
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.decodeText(canonicalKind, text),
          KagemushaWireV1.decodeText(kind, text));
      assertThrows(
          IllegalArgumentException.class,
          () -> KagemushaWireV1.encodeText(kind, new byte[kind.maximumRawBytes() + 1]));
      assertThrows(
          IllegalArgumentException.class,
          () -> KagemushaWireV1.decodeText(kind, text + "A"));
    }
  }

  @Test
  public void delegatedTextCodecRejectsEmptyAndNullInputs() {
    for (final KagemushaWirePayloadKindV1 kind : KagemushaWirePayloadKindV1.values()) {
      final IllegalArgumentException canonicalFailure = assertThrows(
          IllegalArgumentException.class,
          () -> org.hyperledger.iroha.sdk.offline.KagemushaWireV1.encodeText(
              kind.canonicalKind(), new byte[0]));
      final IllegalArgumentException javaFailure = assertThrows(
          IllegalArgumentException.class,
          () -> KagemushaWireV1.encodeText(kind, new byte[0]));
      assertEquals(canonicalFailure.getMessage(), javaFailure.getMessage());
      assertThrows(NullPointerException.class, () -> KagemushaWireV1.encodeText(kind, null));
      assertThrows(NullPointerException.class, () -> KagemushaWireV1.decodeText(kind, null));
    }
    assertThrows(
        NullPointerException.class, () -> KagemushaWireV1.encodeText(null, new byte[] {1}));
    assertThrows(NullPointerException.class, () -> KagemushaWireV1.decodeText(null, "kgm1:AQ"));
  }

  @Test
  public void paymentProofDecoderRejectsOversizedArchivesExactlyLikeKotlin() {
    final byte[] oversized = new byte[KagemushaWireV1.MAXIMUM_PAIRED_PROOF_BYTES + 1];
    final IllegalArgumentException canonicalFailure = assertThrows(
        IllegalArgumentException.class,
        () -> org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
            .decodePaymentProofShapeExact(oversized));
    final IllegalArgumentException javaFailure = assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.decodePaymentProofShapeExact(oversized));
    assertEquals(canonicalFailure.getMessage(), javaFailure.getMessage());
  }

  @Test
  public void standaloneRedemptionProofShapeRoundTripsWithoutPaymentSchemaConfusion() {
    // Synthetic structural components exercise codec boundaries, not proof validity or authority.
    final KagemushaRedemptionProofV1 proof = new KagemushaRedemptionProofV1(
        1,
        repeat(0x21), repeat(0x22), repeat(0x23), repeat(0x24), repeat(0x25),
        repeat(0x26), repeat(0x27), new byte[] {0x31}, new byte[] {0x32},
        Arrays.copyOf(repeat(0x41), KagemushaWireV1.HISTORY_ACCUMULATOR_BYTES),
        Arrays.copyOf(repeat(0x42), KagemushaWireV1.HISTORY_ACCUMULATOR_BYTES));
    final byte[] raw = KagemushaNoritoV1.encodeRedemptionProofShape(proof);
    assertTrue(raw.length <= KagemushaWireV1.MAXIMUM_REDEMPTION_PROOF_BYTES);
    assertArrayEquals(
        org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1.encodeRedemptionProofShape(proof),
        raw);
    assertArrayEquals(
        raw,
        KagemushaNoritoV1.encodeRedemptionProofShape(
            KagemushaNoritoV1.decodeRedemptionProofShapeExact(raw)));
    assertArrayEquals(
        raw,
        KagemushaNoritoV1.encodeRedemptionProofShape(
            org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
                .decodeRedemptionProofShapeExact(raw)));
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.decodePaymentProofShapeExact(raw));
    assertThrows(
        IllegalArgumentException.class,
        () -> org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1.decodePaymentProofShapeExact(raw));

    final byte[] oversized = new byte[KagemushaWireV1.MAXIMUM_REDEMPTION_PROOF_BYTES + 1];
    final IllegalArgumentException canonicalFailure = assertThrows(
        IllegalArgumentException.class,
        () -> org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
            .decodeRedemptionProofShapeExact(oversized));
    final IllegalArgumentException javaFailure = assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.decodeRedemptionProofShapeExact(oversized));
    assertEquals(canonicalFailure.getMessage(), javaFailure.getMessage());
  }

  @Test
  public void unauthenticatedNoCommitClosureHasNoUsableCodecSurface() {
    final Set<String> methods = Arrays.stream(KagemushaNoritoV1.class.getMethods())
        .map(Method::getName)
        .collect(Collectors.toSet());
    assertFalse(methods.contains("encodeNoCommitClosureShape"));
    assertFalse(methods.contains("decodeNoCommitClosureShapeExact"));
    assertFalse(methods.contains("noCommitTerminalSigningBytesShape"));
  }

  @Test
  public void inventoryAndCompactBoundsExactlyMirrorKotlin() {
    assertEquals(6_528, KagemushaWireV1.MAXIMUM_PAIRED_PROOF_BYTES);
    assertEquals(6_528, KagemushaWireV1.MAXIMUM_REDEMPTION_PROOF_BYTES);
    assertEquals(1_024, KagemushaWireV1.MAXIMUM_COMMIT_CERTIFICATE_BYTES);
    assertEquals(37, KagemushaWireV1.PAYMENT_REQUEST_MODE_TRANSCRIPT_BYTES);
    assertEquals(16, KagemushaWireV1.RECEIVE_FOLD_BATCH_SIZE);
    assertEquals(928, KagemushaWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES);
    assertEquals(192, KagemushaWireV1.MAXIMUM_ACCEPTANCE_INTENT_BYTES);
    assertEquals(256, KagemushaWireV1.MAXIMUM_ACCEPTANCE_TICKET_BYTES);
    assertEquals(7_552, KagemushaWireV1.MAXIMUM_PAYMENT_BYTES);
    assertEquals(1_376, KagemushaWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_RAW_BYTES);
    assertEquals(1_851, KagemushaWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_TEXT_BYTES);
    assertEquals(9_211, KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES);
    assertEquals(12_288, KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES);
    assertEquals(64 * 1_024, KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_COMMAND_BYTES);
    assertEquals(128, KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_RESULT_BYTES);
    assertEquals(
        Arrays.asList(
            KagemushaWirePayloadKindV1.PAYMENT_REQUEST,
            KagemushaWirePayloadKindV1.ACCEPTANCE_INTENT,
            KagemushaWirePayloadKindV1.ACCEPTANCE_TICKET,
            KagemushaWirePayloadKindV1.PAYMENT,
            KagemushaWirePayloadKindV1.ACKNOWLEDGEMENT,
            KagemushaWirePayloadKindV1.NO_COMMIT_CLOSURE,
            KagemushaWirePayloadKindV1.MINT_AUTHORIZATION,
            KagemushaWirePayloadKindV1.MINT_CREDIT,
            KagemushaWirePayloadKindV1.REDEMPTION_VOUCHER),
        Arrays.asList(KagemushaWirePayloadKindV1.values()));
    assertEquals(
        Arrays.toString(org.hyperledger.iroha.sdk.offline.KagemushaWirePayloadKindV1.values()),
        Arrays.toString(KagemushaWirePayloadKindV1.values()));
    assertEquals(
        org.hyperledger.iroha.sdk.offline.KagemushaWireV1.HANDOFF_CAPABILITY,
        KagemushaWireV1.HANDOFF_CAPABILITY);
    assertEquals(
        org.hyperledger.iroha.sdk.offline.KagemushaWireV1.TEXT_PREFIX,
        KagemushaWireV1.TEXT_PREFIX);
    assertEquals(
        org.hyperledger.iroha.sdk.offline.KagemushaWireV1.REQUEST_MAX_TTL_MS,
        KagemushaWireV1.REQUEST_MAX_TTL_MS);
    assertArrayEquals(
        new int[] {
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.WIRE_VERSION,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.DEVICE_LIFECYCLE_VERSION,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_RAW_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_TEXT_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_PAIRED_PROOF_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_REDEMPTION_PROOF_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_COMMIT_CERTIFICATE_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_CURRENT_PROOFS_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_PARITY_PROOF_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.HISTORY_ACCUMULATOR_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.PAYMENT_OUTBOX_MIN_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.REDEMPTION_OUTBOX_MIN_BYTES,
          org.hyperledger.iroha.sdk.offline.KagemushaWireV1.ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES
        },
        new int[] {
          KagemushaWireV1.WIRE_VERSION,
          KagemushaWireV1.DEVICE_LIFECYCLE_VERSION,
          KagemushaWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_RAW_BYTES,
          KagemushaWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_TEXT_BYTES,
          KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES,
          KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES,
          KagemushaWireV1.MAXIMUM_PAIRED_PROOF_BYTES,
          KagemushaWireV1.MAXIMUM_REDEMPTION_PROOF_BYTES,
          KagemushaWireV1.MAXIMUM_COMMIT_CERTIFICATE_BYTES,
          KagemushaWireV1.MAXIMUM_CURRENT_PROOFS_BYTES,
          KagemushaWireV1.MAXIMUM_PARITY_PROOF_BYTES,
          KagemushaWireV1.HISTORY_ACCUMULATOR_BYTES,
          KagemushaWireV1.PAYMENT_OUTBOX_MIN_BYTES,
          KagemushaWireV1.REDEMPTION_OUTBOX_MIN_BYTES,
          KagemushaWireV1.ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES
        });
  }

  @Test
  public void peerAllocationLimitsAreBoundedByThePostCommitPayment() {
    assertEquals(
        KagemushaWireV1.MAXIMUM_PAYMENT_BYTES,
        IrohaPeerWireLimitsV1.PEER_V1.maximumCanonicalBytes());
    assertEquals(
        KagemushaWireV1.MAXIMUM_PAYMENT_BYTES,
        IrohaPeerWireLimitsV1.PEER_V1.maximumKagemushaEncodedBytes());
    assertEquals(KagemushaWireV1.MAXIMUM_PAYMENT_BYTES, IrohaPeerWireMessageV1.MAXIMUM_CANONICAL_BYTES);
    assertEquals(
        KagemushaWireV1.MAXIMUM_PAYMENT_BYTES, IrohaPeerWireMessageV1.MAXIMUM_KAGEMUSHA_ENCODED_BYTES);
    final IrohaPeerWireLimitsV1 minimum = new IrohaPeerWireLimitsV1(1, 1);
    assertEquals(1, minimum.maximumCanonicalBytes());
    assertEquals(1, minimum.maximumKagemushaEncodedBytes());
    assertThrows(IllegalArgumentException.class, () -> new IrohaPeerWireLimitsV1(0, 1));
    assertThrows(IllegalArgumentException.class, () -> new IrohaPeerWireLimitsV1(1, 0));
    assertThrows(
        IllegalArgumentException.class,
        () -> new IrohaPeerWireLimitsV1(KagemushaWireV1.MAXIMUM_PAYMENT_BYTES + 1, 1));
    assertThrows(
        IllegalArgumentException.class,
        () -> new IrohaPeerWireLimitsV1(1, KagemushaWireV1.MAXIMUM_PAYMENT_BYTES + 1));
  }

  @Test
  public void compactUnprovedIntentUsesOnlyTheSecondPeerKind() {
    final KagemushaAcceptanceIntentV1 intent = new KagemushaAcceptanceIntentV1(
        1, repeat(0x21), repeat(0x22), BigInteger.TEN, repeat(0x23));
    final byte[] raw = KagemushaNoritoV1.encodeAcceptanceIntentShape(intent);
    assertTrue(raw.length <= KagemushaWireV1.MAXIMUM_ACCEPTANCE_INTENT_BYTES);
    assertPeerRoundTrip(IrohaPeerPayloadKind.INTENT, raw);
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerKagemushaAdapterV1.wrap(IrohaPeerPayloadKind.REQUEST, raw));
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerKagemushaAdapterV1.wrap(IrohaPeerPayloadKind.TICKET, raw));
  }

  @Test
  public void peerWireHasExactlyFiveMessageKinds() {
    assertEquals(
        Arrays.asList("REQUEST", "INTENT", "TICKET", "PAYMENT", "ACKNOWLEDGEMENT"),
        Arrays.stream(IrohaPeerPayloadKind.values()).map(Enum::name).collect(Collectors.toList()));
    assertEquals(
        Arrays.asList(1, 2, 3, 4, 5),
        Arrays.stream(IrohaPeerPayloadKind.values())
            .map(IrohaPeerPayloadKind::code)
            .collect(Collectors.toList()));
    for (final IrohaPeerPayloadKind kind : IrohaPeerPayloadKind.values()) {
      assertEquals(
          org.hyperledger.iroha.sdk.offline.KagemushaIpm1PayloadKindV1.valueOf(kind.name()).wireTag,
          kind.code());
      assertEquals(kind, IrohaPeerPayloadKind.fromCode(kind.code()));
    }
  }

  @Test
  public void retiredThreeMessageWireSurfaceFailsClosed() throws Exception {
    assertNull(IrohaPeerPayloadKind.fromCode(0));
    assertNull(IrohaPeerPayloadKind.fromCode(6));
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerPayloadKind.valueOf("RECEIVE" + "_REQUEST"));
    assertThrows(
        NoSuchFieldException.class,
        () -> KagemushaWireV1.class.getField("MAXIMUM_" + "SESSION_RAW_BYTES"));
    assertThrows(
        NoSuchFieldException.class,
        () -> KagemushaWireV1.class.getField("MAXIMUM_" + "SESSION_TEXT_BYTES"));
    assertThrows(
        NoSuchFieldException.class,
        () -> KagemushaWireV1.class.getField("MAXIMUM_TERMINAL_" + "HANDOFF_RAW_BYTES"));
    assertThrows(
        NoSuchFieldException.class,
        () -> KagemushaWireV1.class.getField("MAXIMUM_TERMINAL_" + "HANDOFF_TEXT_BYTES"));

    final String fixture = loadCanonicalFixture();
    final byte[] oldTagTwoPayment = fixtureHex(fixture, "payment");
    final byte[] oldTagThreeAcknowledgement = fixtureHex(fixture, "acknowledgement");
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.KAGEMUSHA_V1,
                IrohaPeerPayloadKind.INTENT,
                IrohaPeerKagemushaAdapterV1.ARCHIVE_SCHEMA_VERSION,
                oldTagTwoPayment));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.KAGEMUSHA_V1,
                IrohaPeerPayloadKind.TICKET,
                IrohaPeerKagemushaAdapterV1.ARCHIVE_SCHEMA_VERSION,
                oldTagThreeAcknowledgement));
  }

  @Test
  public void peerWireEnforcesEveryMessageSpecificCanonicalBound() {
    final IrohaPeerPayloadKind[] kinds = IrohaPeerPayloadKind.values();
    final int[] bounds = {
      KagemushaWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES,
      KagemushaWireV1.MAXIMUM_ACCEPTANCE_INTENT_BYTES,
      KagemushaWireV1.MAXIMUM_ACCEPTANCE_TICKET_BYTES,
      KagemushaWireV1.MAXIMUM_PAYMENT_BYTES,
      KagemushaWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES
    };
    assertEquals(kinds.length, bounds.length);
    for (int index = 0; index < kinds.length; index++) {
      final IrohaPeerPayloadKind kind = kinds[index];
      final IllegalArgumentException failure =
          assertThrows(
              IllegalArgumentException.class,
              () ->
                  new IrohaPeerCanonicalPayload(
                      IrohaPeerPayloadProfile.KAGEMUSHA_V1,
                      kind,
                      IrohaPeerKagemushaAdapterV1.ARCHIVE_SCHEMA_VERSION,
                      new byte[bounds[kind.ordinal()] + 1]));
      assertEquals("Peer payload exceeds the frozen " + kind + " bound", failure.getMessage());
    }
  }

  @Test
  public void publicStatementsContainNoPredecessorSuccessorLaneOrCommitInstant() {
    final Set<String> forbidden =
        new HashSet<>(
            Arrays.asList(
                "beforecommitment",
                "aftercommitment",
                "predecessorstatecommitment",
                "successorstatecommitment",
                "recipientlane",
                "committedat",
                "hardwaretransitioncommitment"));
    for (final Class<?> type :
        Arrays.asList(KagemushaPaymentOutputV1.class, KagemushaRedemptionStatementV1.class)) {
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
    assertTrue(requestMembers.contains("requestMode"));
    assertFalse(requestMembers.contains("recipientOneTimeKey"));
    assertFalse(requestMembers.contains("amount"));
    assertFalse(requestMembers.contains("recipientLaneId"));
    assertFalse(requestMembers.contains("recipientEncryptionKey"));

    final Set<String> ticketMembers = memberNames(KagemushaAcceptanceTicketV1.class);
    assertTrue(ticketMembers.contains("recipientOneTimeKey"));

    final Set<String> outputMembers = memberNames(KagemushaPaymentOutputV1.class);
    assertTrue(outputMembers.contains("acceptanceIntentDigest"));
    assertFalse(outputMembers.contains("intentAuthorizationId"));
    assertFalse(outputMembers.contains("intentAuthorizationDigest"));
    assertTrue(outputMembers.contains("acceptanceTicketDigest"));
    assertTrue(outputMembers.contains("transitionNullifier"));
    assertTrue(outputMembers.contains("creditId"));
    assertTrue(outputMembers.contains("ciphertextCommitment"));
    assertTrue(outputMembers.contains("commitEvidence"));

    final Set<String> paymentMembers = memberNames(KagemushaPaymentV1.class);
    assertTrue(paymentMembers.contains("output"));
    assertTrue(paymentMembers.contains("encryptedCredit"));
    assertTrue(paymentMembers.contains("commitCertificate"));
    assertTrue(paymentMembers.contains("proof"));
    assertFalse(paymentMembers.contains("terminalSignature"));

    final Set<String> redemptionMembers = memberNames(KagemushaRedemptionStatementV1.class);
    assertTrue(redemptionMembers.contains("terminalNullifier"));
    assertTrue(redemptionMembers.contains("redemptionCommitment"));
    assertTrue(redemptionMembers.contains("commitEvidence"));
    final Set<String> voucherMembers = memberNames(KagemushaRedemptionVoucherV1.class);
    assertTrue(voucherMembers.contains("commitCertificate"));
    assertTrue(voucherMembers.contains("proof"));
  }

  @Test
  public void intentAndCreditIdApisHaveNoAuthorizationOrTicketAliases() throws Exception {
    KagemushaNoritoV1.class.getMethod(
        "acceptanceIntentDigestShape",
        KagemushaAcceptanceIntentV1.class,
        KagemushaPaymentRequestV1.class);
    for (final Method method : KagemushaNoritoV1.class.getMethods()) {
      assertFalse(method.getName().startsWith("acceptanceIntent" + "Authorization"));
    }
    KagemushaNoritoV1.class.getMethod(
        "expectedPeerCreditIdShape",
        KagemushaPaymentOutputV1.class,
        KagemushaPaymentRequestV1.class,
        KagemushaAcceptanceIntentV1.class);
    assertThrows(
        NoSuchMethodException.class,
        () ->
            KagemushaNoritoV1.class.getMethod(
                "expectedPeerCreditIdShape",
                KagemushaPaymentOutputV1.class,
                KagemushaPaymentRequestV1.class,
                KagemushaAcceptanceIntentV1.class,
                KagemushaAcceptanceTicketV1.class));
    KagemushaNoritoV1.class.getMethod(
        "preparedTransferDigestShape",
        KagemushaPaymentRequestV1.class,
        KagemushaAcceptanceIntentV1.class,
        KagemushaAcceptanceTicketV1.class,
        byte[].class,
        byte[].class);
    KagemushaNoritoV1.class.getMethod(
        "peerCreditContextShape",
        KagemushaPaymentRequestV1.class,
        KagemushaAcceptanceIntentV1.class,
        KagemushaAcceptanceTicketV1.class,
        KagemushaPaymentOutputV1.class);
    assertEquals(
        32,
        KagemushaNoritoV1.peerCreditOpeningCommitmentShape(
                repeat(0x71),
                new org.hyperledger.iroha.sdk.offline.KagemushaX25519PublicKeyV1(
                    repeat(0x72)),
                BigInteger.valueOf(5),
                repeat(0x73),
                repeat(0x74),
                repeat(0x75))
            .length);
  }

  @Test
  public void deviceMintStageCodecSurfaceMirrorsKotlin() throws Exception {
    KagemushaNoritoV1.class.getMethod(
        "encodeDeviceMintStageCommandShape", KagemushaDeviceMintStageCommandV1.class);
    KagemushaNoritoV1.class.getMethod(
        "encodeDeviceMintStageCommandShape", byte[].class, byte[].class);
    KagemushaNoritoV1.class.getMethod(
        "decodeDeviceMintStageCommandShapeExact", byte[].class);
    KagemushaNoritoV1.class.getMethod(
        "encodeDeviceMintStageResultShape", KagemushaDeviceMintStageResultV1.class);
    KagemushaNoritoV1.class.getMethod(
        "decodeDeviceMintStageResultShapeExact", byte[].class);
    KagemushaNoritoV1.class.getMethod(
        "decodeDeviceMintStageResultShapeExact",
        byte[].class,
        KagemushaDeviceMintStageCommandV1.class);
  }

  @Test
  public void sharedCanonicalDeviceMintStageFixtureRoundTripsByteForByte() throws Exception {
    final String fixture =
        loadCanonicalFixture("fixtures/offline/kagemusha_device_mint_stage_v1.json");
    assertEquals(1L, fixtureLong(fixture, null, "fixture_version"));
    assertEquals("KAGEMUSHA", fixtureString(fixture, null, "protocol"));
    assertEquals(21L, fixtureLong(fixture, null, "operation"));
    assertTrue(fixtureBoolean(fixture, null, "structural_only"));

    final byte[] authorizationBytes = fixtureHexValue(fixture, "authorization", "hex");
    final byte[] creditBytes = fixtureHexValue(fixture, "mint_credit", "hex");
    final byte[] commandBytes = fixtureHexValue(fixture, "command", "hex");
    final byte[] stagedBytes = fixtureHexValue(fixture, "staged_result", "hex");
    final byte[] duplicateBytes = fixtureHexValue(fixture, "exact_duplicate_result", "hex");
    final String[] sections = {
      "authorization", "mint_credit", "command", "staged_result", "exact_duplicate_result"
    };
    final byte[][] rawSections = {
      authorizationBytes, creditBytes, commandBytes, stagedBytes, duplicateBytes
    };
    for (int index = 0; index < sections.length; index++) {
      assertEquals(fixtureLong(fixture, sections[index], "raw_bytes"), rawSections[index].length);
    }
    final KagemushaMintAuthorizationV1 authorization =
        KagemushaNoritoV1.decodeMintAuthorizationShapeExact(authorizationBytes);
    final KagemushaMintCreditV1 credit =
        KagemushaNoritoV1.decodeMintCreditShapeExact(creditBytes, authorization);
    assertArrayEquals(
        authorizationBytes, KagemushaNoritoV1.encodeMintAuthorizationShape(authorization));
    assertArrayEquals(creditBytes, KagemushaNoritoV1.encodeMintCreditShape(credit, authorization));

    assertEquals(
        "iroha_data_model::kagemusha::kagemusha_device_v1::KagemushaDeviceMintStageCommandV1",
        fixtureString(fixture, "command", "schema"));
    assertEquals(8L, fixtureLong(fixture, "command", "alignment"));
    assertTrue(commandBytes.length <= KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_COMMAND_BYTES);
    final KagemushaDeviceMintStageCommandV1 command =
        KagemushaNoritoV1.decodeDeviceMintStageCommandShapeExact(commandBytes);
    assertEquals(1, command.version);
    assertArrayEquals(authorizationBytes, command.canonicalAuthorization());
    assertArrayEquals(creditBytes, command.canonicalMintCredit());
    assertArrayEquals(commandBytes, KagemushaNoritoV1.encodeDeviceMintStageCommandShape(command));
    assertArrayEquals(
        commandBytes,
        KagemushaNoritoV1.encodeDeviceMintStageCommandShape(authorizationBytes, creditBytes));

    final byte[] creditId = fixtureHexValue(fixture, null, "credit_id_hex");
    assertArrayEquals(creditId, authorization.statement.creditId());
    assertArrayEquals(creditId, credit.statement.lifecycle.creditId());
    final String[] resultSections = {"staged_result", "exact_duplicate_result"};
    final byte[][] resultBytes = {stagedBytes, duplicateBytes};
    final int[] dispositions = {
      KagemushaDeviceMintStageResultV1.STAGED,
      KagemushaDeviceMintStageResultV1.EXACT_DUPLICATE
    };
    for (int index = 0; index < resultSections.length; index++) {
      assertEquals(
          "iroha_data_model::kagemusha::kagemusha_device_v1::KagemushaDeviceMintStageResultV1",
          fixtureString(fixture, resultSections[index], "schema"));
      assertEquals(2L, fixtureLong(fixture, resultSections[index], "alignment"));
      final byte[] bytes = resultBytes[index];
      assertTrue(bytes.length <= KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_RESULT_BYTES);
      for (final KagemushaDeviceMintStageResultV1 result :
          Arrays.asList(
              KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(bytes),
              KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(bytes, command))) {
        assertEquals(1, result.version);
        assertEquals(dispositions[index], result.disposition);
        assertArrayEquals(creditId, result.creditId());
        assertArrayEquals(bytes, KagemushaNoritoV1.encodeDeviceMintStageResultShape(result));
      }
    }
  }

  @Test
  public void deviceMintStageRejectsUnsafeHeadersBeforeGenericDecoding() throws Exception {
    final String fixture =
        loadCanonicalFixture("fixtures/offline/kagemusha_device_mint_stage_v1.json");
    for (final String section :
        Arrays.asList("command", "staged_result", "exact_duplicate_result")) {
      final byte[] canonical = fixtureHexValue(fixture, section, "hex");
      final int maximum = section.equals("command")
          ? KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_COMMAND_BYTES
          : KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_RESULT_BYTES;
      final Consumer<byte[]> decode = section.equals("command")
          ? KagemushaNoritoV1::decodeDeviceMintStageCommandShapeExact
          : KagemushaNoritoV1::decodeDeviceMintStageResultShapeExact;
      for (final long payloadLength : new long[] {1L, Integer.MAX_VALUE}) {
        final byte[] compressed = mintStageHeader(
            canonical, NoritoHeader.COMPRESSION_ZSTD, NoritoHeader.COMPACT_LEN, payloadLength);
        assertEquals(41, compressed.length);
        assertEquals(
            "KAGEMUSHA V1 canonical archive must be uncompressed",
            assertThrows(IllegalArgumentException.class, () -> decode.accept(compressed))
                .getMessage());
      }
      for (final int flags :
          new int[] {0, NoritoHeader.COMPACT_LEN | NoritoHeader.PACKED_SEQ}) {
        final byte[] packed = mintStageHeader(
            canonical, NoritoHeader.COMPRESSION_NONE, flags, Integer.MAX_VALUE);
        assertEquals(
            "KAGEMUSHA V1 canonical archive has noncanonical layout flags",
            assertThrows(IllegalArgumentException.class, () -> decode.accept(packed)).getMessage());
      }
      for (final long payloadLength :
          new long[] {maximum, Integer.MAX_VALUE, 1L << 32, Long.MAX_VALUE, -1L}) {
        final byte[] oversized = mintStageHeader(
            canonical, NoritoHeader.COMPRESSION_NONE, NoritoHeader.COMPACT_LEN, payloadLength);
        assertEquals(
            "KAGEMUSHA V1 declared payload is oversized",
            assertThrows(IllegalArgumentException.class, () -> decode.accept(oversized))
                .getMessage());
      }
      for (final long difference : new long[] {-1L, 1L}) {
        final byte[] mismatched = canonical.clone();
        final ByteBuffer header = ByteBuffer.wrap(mismatched).order(ByteOrder.LITTLE_ENDIAN);
        header.putLong(23, header.getLong(23) + difference);
        assertEquals(
            "KAGEMUSHA V1 payload length does not match canonical framing",
            assertThrows(IllegalArgumentException.class, () -> decode.accept(mismatched))
                .getMessage());
      }
      assertEquals(
          "KAGEMUSHA V1 archive has a truncated Norito header",
          assertThrows(
              IllegalArgumentException.class,
              () -> decode.accept(Arrays.copyOf(canonical, NoritoHeader.HEADER_LENGTH - 1)))
              .getMessage());
    }

    final byte[] authorization = fixtureHexValue(fixture, "authorization", "hex");
    final byte[] credit = fixtureHexValue(fixture, "mint_credit", "hex");
    final byte[] compressedAuthorization = mintStageHeader(
        authorization, NoritoHeader.COMPRESSION_ZSTD, NoritoHeader.COMPACT_LEN, Integer.MAX_VALUE);
    final byte[] compressedCredit = mintStageHeader(
        credit, NoritoHeader.COMPRESSION_ZSTD, NoritoHeader.COMPACT_LEN, Integer.MAX_VALUE);
    assertEquals(
        "KAGEMUSHA V1 canonical archive must be uncompressed",
        assertThrows(
            IllegalArgumentException.class,
            () -> KagemushaNoritoV1.encodeDeviceMintStageCommandShape(compressedAuthorization, credit))
            .getMessage());
    assertEquals(
        "KAGEMUSHA V1 canonical archive must be uncompressed",
        assertThrows(
            IllegalArgumentException.class,
            () -> KagemushaNoritoV1.encodeDeviceMintStageCommandShape(authorization, compressedCredit))
            .getMessage());
  }

  @Test
  public void deviceMintStageCommandRoundTripsThroughEveryJavaOverload() throws Exception {
    final KagemushaDeviceMintStageCommandV1 command = mintStageCommand(0x77);
    final byte[] authorization = command.canonicalAuthorization();
    final byte[] credit = command.canonicalMintCredit();
    final byte[] encoded = KagemushaNoritoV1.encodeDeviceMintStageCommandShape(command);
    assertTrue(encoded.length <= KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_COMMAND_BYTES);
    assertArrayEquals(
        org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
            .encodeDeviceMintStageCommandShape(command),
        encoded);
    assertArrayEquals(
        encoded, KagemushaNoritoV1.encodeDeviceMintStageCommandShape(authorization, credit));
    assertArrayEquals(
        encoded,
        org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
            .encodeDeviceMintStageCommandShape(authorization, credit));
    for (final KagemushaDeviceMintStageCommandV1 decoded :
        Arrays.asList(
            KagemushaNoritoV1.decodeDeviceMintStageCommandShapeExact(encoded),
            org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
                .decodeDeviceMintStageCommandShapeExact(encoded))) {
      assertEquals(1, decoded.version);
      assertArrayEquals(authorization, decoded.canonicalAuthorization());
      assertArrayEquals(credit, decoded.canonicalMintCredit());
      assertArrayEquals(encoded, KagemushaNoritoV1.encodeDeviceMintStageCommandShape(decoded));
    }

    final KagemushaDeviceMintStageCommandV1 copied =
        new KagemushaDeviceMintStageCommandV1(1, authorization, credit);
    authorization[0] ^= 1;
    credit[0] ^= 1;
    copied.canonicalAuthorization()[0] ^= 1;
    copied.canonicalMintCredit()[0] ^= 1;
    assertArrayEquals(encoded, KagemushaNoritoV1.encodeDeviceMintStageCommandShape(copied));
  }

  @Test
  public void deviceMintStageResultsRoundTripBothDispositionsAndBindToTheCommand()
      throws Exception {
    final KagemushaDeviceMintStageCommandV1 command = mintStageCommand(0x77);
    final byte[] creditId =
        KagemushaNoritoV1.decodeMintCreditShapeExact(command.canonicalMintCredit())
            .statement.lifecycle.creditId();
    for (final int disposition :
        new int[] {
          KagemushaDeviceMintStageResultV1.STAGED,
          KagemushaDeviceMintStageResultV1.EXACT_DUPLICATE
        }) {
      final KagemushaDeviceMintStageResultV1 result =
          new KagemushaDeviceMintStageResultV1(1, disposition, creditId);
      final byte[] encoded = KagemushaNoritoV1.encodeDeviceMintStageResultShape(result);
      assertTrue(encoded.length <= KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_RESULT_BYTES);
      assertArrayEquals(
          org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
              .encodeDeviceMintStageResultShape(result),
          encoded);
      for (final KagemushaDeviceMintStageResultV1 decoded :
          Arrays.asList(
              KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(encoded),
              KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(encoded, command),
              org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
                  .decodeDeviceMintStageResultShapeExact(encoded),
              org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
                  .decodeDeviceMintStageResultShapeExact(encoded, command))) {
        assertEquals(1, decoded.version);
        assertEquals(disposition, decoded.disposition);
        assertArrayEquals(creditId, decoded.creditId());
        assertArrayEquals(encoded, KagemushaNoritoV1.encodeDeviceMintStageResultShape(decoded));
      }
    }
    for (final int disposition : new int[] {-1, 2, 255}) {
      assertThrows(
          IllegalArgumentException.class,
          () -> new KagemushaDeviceMintStageResultV1(1, disposition, creditId));
    }
  }

  @Test
  public void deviceMintStageRejectsSubstitutedAuthorizationAndResultCredit() throws Exception {
    final KagemushaDeviceMintStageCommandV1 command = mintStageCommand(0x77);
    final KagemushaDeviceMintStageCommandV1 other = mintStageCommand(0x76);
    final KagemushaDeviceMintStageCommandV1 substituted =
        new KagemushaDeviceMintStageCommandV1(
            1, command.canonicalAuthorization(), other.canonicalMintCredit());
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.encodeDeviceMintStageCommandShape(substituted));
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.encodeDeviceMintStageCommandShape(
            substituted.canonicalAuthorization(), substituted.canonicalMintCredit()));

    final byte[] otherCreditId =
        KagemushaNoritoV1.decodeMintCreditShapeExact(other.canonicalMintCredit())
            .statement.lifecycle.creditId();
    for (final int disposition :
        new int[] {
          KagemushaDeviceMintStageResultV1.STAGED,
          KagemushaDeviceMintStageResultV1.EXACT_DUPLICATE
        }) {
      final byte[] result = KagemushaNoritoV1.encodeDeviceMintStageResultShape(
          new KagemushaDeviceMintStageResultV1(1, disposition, otherCreditId));
      assertArrayEquals(
          otherCreditId, KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(result).creditId());
      final IllegalArgumentException canonicalFailure = assertThrows(
          IllegalArgumentException.class,
          () -> org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
              .decodeDeviceMintStageResultShapeExact(result, command));
      final IllegalArgumentException javaFailure = assertThrows(
          IllegalArgumentException.class,
          () -> KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(result, command));
      assertEquals(canonicalFailure.getMessage(), javaFailure.getMessage());
      assertThrows(
          IllegalArgumentException.class,
          () -> KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(result, substituted));
    }
  }

  @Test
  public void deviceMintStageRejectsTrailingOuterAndNestedArchives() throws Exception {
    final KagemushaDeviceMintStageCommandV1 command = mintStageCommand(0x77);
    final byte[] authorization = command.canonicalAuthorization();
    final byte[] credit = command.canonicalMintCredit();
    final byte[] commandBytes = KagemushaNoritoV1.encodeDeviceMintStageCommandShape(command);
    final byte[] resultBytes = KagemushaNoritoV1.encodeDeviceMintStageResultShape(
        new KagemushaDeviceMintStageResultV1(
            1,
            KagemushaDeviceMintStageResultV1.STAGED,
            KagemushaNoritoV1.decodeMintCreditShapeExact(credit).statement.lifecycle.creditId()));
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.decodeDeviceMintStageCommandShapeExact(
            Arrays.copyOf(commandBytes, commandBytes.length + 1)));
    final byte[] trailingResult = Arrays.copyOf(resultBytes, resultBytes.length + 1);
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(trailingResult));
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(trailingResult, command));
    for (final KagemushaDeviceMintStageCommandV1 trailing :
        Arrays.asList(
            new KagemushaDeviceMintStageCommandV1(
                1, Arrays.copyOf(authorization, authorization.length + 1), credit),
            new KagemushaDeviceMintStageCommandV1(
                1, authorization, Arrays.copyOf(credit, credit.length + 1)))) {
      assertThrows(
          IllegalArgumentException.class,
          () -> KagemushaNoritoV1.encodeDeviceMintStageCommandShape(trailing));
      assertThrows(
          IllegalArgumentException.class,
          () -> KagemushaNoritoV1.encodeDeviceMintStageCommandShape(
              trailing.canonicalAuthorization(), trailing.canonicalMintCredit()));
    }
  }

  @Test
  public void deviceMintStageEnforcesOuterAndNestedByteCaps() throws Exception {
    final KagemushaDeviceMintStageCommandV1 command = mintStageCommand(0x77);
    final byte[] oversizedCommand =
        new byte[KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_COMMAND_BYTES + 1];
    final byte[] oversizedResult =
        new byte[KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_RESULT_BYTES + 1];
    final IllegalArgumentException canonicalCommandFailure = assertThrows(
        IllegalArgumentException.class,
        () -> org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
            .decodeDeviceMintStageCommandShapeExact(oversizedCommand));
    final IllegalArgumentException javaCommandFailure = assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.decodeDeviceMintStageCommandShapeExact(oversizedCommand));
    assertEquals(canonicalCommandFailure.getMessage(), javaCommandFailure.getMessage());
    final IllegalArgumentException canonicalResultFailure = assertThrows(
        IllegalArgumentException.class,
        () -> org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
            .decodeDeviceMintStageResultShapeExact(oversizedResult));
    final IllegalArgumentException javaResultFailure = assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(oversizedResult));
    assertEquals(canonicalResultFailure.getMessage(), javaResultFailure.getMessage());
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(oversizedResult, command));

    final KagemushaDeviceMintStageCommandV1 maximumFields =
        new KagemushaDeviceMintStageCommandV1(
            1,
            new byte[KagemushaWireV1.MAXIMUM_MINT_AUTHORIZATION_BYTES],
            new byte[KagemushaWireV1.MAXIMUM_MINT_CREDIT_BYTES]);
    assertEquals(
        KagemushaWireV1.MAXIMUM_MINT_AUTHORIZATION_BYTES,
        maximumFields.canonicalAuthorization().length);
    assertEquals(
        KagemushaWireV1.MAXIMUM_MINT_CREDIT_BYTES, maximumFields.canonicalMintCredit().length);
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.encodeDeviceMintStageCommandShape(
            new byte[KagemushaWireV1.MAXIMUM_MINT_AUTHORIZATION_BYTES + 1],
            command.canonicalMintCredit()));
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNoritoV1.encodeDeviceMintStageCommandShape(
            command.canonicalAuthorization(),
            new byte[KagemushaWireV1.MAXIMUM_MINT_CREDIT_BYTES + 1]));
  }

  private static KagemushaDeviceMintStageCommandV1 mintStageCommand(final int operationTag)
      throws Exception {
    // Reuse shared fixture identities/envelope; the synthetic paired proofs test shape, not authority.
    // This follows the mint construction in Kotlin's KagemushaV1TestSupport.
    final String fixture = loadCanonicalFixture();
    final KagemushaPaymentRequestV1 request =
        KagemushaNoritoV1.decodePaymentRequestShapeExact(fixtureHex(fixture, "payment_request"));
    final KagemushaAcceptanceIntentV1 intent = KagemushaNoritoV1.decodeAcceptanceIntentShapeExact(
        fixtureHex(fixture, "acceptance_intent"), request);
    final KagemushaAcceptanceTicketV1 ticket = KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
        fixtureHex(fixture, "acceptance_ticket"), request, intent);
    final byte[] encrypted = KagemushaNoritoV1.decodePaymentShapeExact(
        fixtureHex(fixture, "payment"), request, intent, ticket).encryptedCredit();
    final KagemushaMintAuthorizationContextV1 context = new KagemushaMintAuthorizationContextV1(
        1, repeat(operationTag), request.releaseId(), request.hardwareCredential.suiteId(),
        repeat(0x78), repeat(0x79), request.networkId, request.asset, request.assetIncarnation,
        request.scale, request.liabilityPoolId(), BigInteger.valueOf(40), request.recipient,
        request.recipient, request.hardwareCredential.credentialId(),
        request.hardwareCredential.hardwareProfileId(), request.hardwareCredential.policyEpoch,
        repeat(0x7a), repeat(0x7b), ticket.recipientOneTimeKey);
    final byte[] ciphertextDigest = KagemushaNoritoV1.ciphertextDigestShape(encrypted);
    final byte[] creditId = KagemushaNoritoV1.expectedMintCreditIdShape(
        mintStageStatement(context, repeat(0x7e), ciphertextDigest, repeat(0x7f)));
    final KagemushaMintAuthorizationStatementV1 authorizationStatement =
        new KagemushaMintAuthorizationStatementV1(
            1, context, repeat(0x7d), creditId, ciphertextDigest);
    final KagemushaMintAuthorizationV1 authorization = new KagemushaMintAuthorizationV1(
        1, authorizationStatement, mintStageProof(
            KagemushaNoritoV1.mintAuthorizationStatementDigestShape(authorizationStatement), 0x80));
    final KagemushaMintCreditStatementV1 statement = mintStageStatement(
        context, creditId, ciphertextDigest, KagemushaNoritoV1.mintAuthorizationDigestShape(authorization));
    final KagemushaMintCreditV1 credit = new KagemushaMintCreditV1(
        1, statement, mintStageProof(KagemushaNoritoV1.mintCreditStatementDigestShape(statement), 0x90),
        repeat(0x91), repeat(0x92), repeat(0x93), repeat(0x94), encrypted,
        context.artifactManifestDigest());
    return new KagemushaDeviceMintStageCommandV1(
        1,
        KagemushaNoritoV1.encodeMintAuthorizationShape(authorization),
        KagemushaNoritoV1.encodeMintCreditShape(credit, authorization));
  }

  private static KagemushaMintCreditStatementV1 mintStageStatement(
      final KagemushaMintAuthorizationContextV1 context,
      final byte[] creditId,
      final byte[] ciphertextDigest,
      final byte[] authorizationDigest) {
    final KagemushaLifecycleBindingV1 lifecycle = new KagemushaLifecycleBindingV1(
        1, context.networkId, 1, context.suiteId(), context.vkDigest(), context.releaseId(),
        context.asset, context.assetIncarnation, context.scale, context.liabilityPoolId(),
        context.hardwareProfileId(), context.policyEpoch, KagemushaOperationKindV1.MINT_FOLD,
        new byte[32], new byte[32], creditId, ciphertextDigest);
    return new KagemushaMintCreditStatementV1(
        1, lifecycle, context.recipientCredentialCommitment(),
        KagemushaNoritoV1.mintAuthorizationContextDigestShape(context), authorizationDigest,
        context.amount, repeat(0x7d), context.recipient, context.creditCommitment(), 1_500);
  }

  private static KagemushaPairedProofV1 mintStageProof(final byte[] semanticDigest, final int tag) {
    final byte[] eqHistory = new byte[KagemushaWireV1.HISTORY_ACCUMULATOR_BYTES];
    final byte[] epHistory = new byte[KagemushaWireV1.HISTORY_ACCUMULATOR_BYTES];
    Arrays.fill(eqHistory, (byte) tag);
    Arrays.fill(epHistory, (byte) (tag + 1));
    return new KagemushaPairedProofV1(
        1, repeat(tag), repeat(tag + 1), semanticDigest, repeat(tag + 2), repeat(tag + 3),
        repeat(tag + 4), repeat(tag + 5), new byte[] {(byte) tag}, new byte[] {(byte) (tag + 1)},
        eqHistory, epHistory);
  }

  private static Set<String> memberNames(final Class<?> type) {
    final Set<String> names =
        Arrays.stream(type.getDeclaredMethods()).map(Method::getName).collect(Collectors.toSet());
    names.addAll(
        Arrays.stream(type.getDeclaredFields()).map(field -> field.getName()).collect(Collectors.toSet()));
    return names;
  }

  private static byte[] repeat(final int value) {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static byte[] mintStageHeader(
      final byte[] canonical,
      final int compression,
      final int flags,
      final long payloadLength) {
    // Keep only the schema-bearing header and one malformed body byte, never the declared size.
    final byte[] bytes = Arrays.copyOf(canonical, NoritoHeader.HEADER_LENGTH + 1);
    bytes[22] = (byte) compression;
    ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).putLong(23, payloadLength);
    bytes[NoritoHeader.HEADER_LENGTH - 1] = (byte) flags;
    return bytes;
  }

  private static byte[] fixtureHexValue(
      final String fixture, final String section, final String field) {
    final String hex = fixtureString(fixture, section, field);
    final byte[] bytes = new byte[hex.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] =
          (byte)
              ((Character.digit(hex.charAt(index * 2), 16) << 4)
                  | Character.digit(hex.charAt(index * 2 + 1), 16));
    }
    return bytes;
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder value = new StringBuilder(bytes.length * 2);
    for (final byte octet : bytes) {
      value.append(String.format(Locale.ROOT, "%02x", octet & 0xff));
    }
    return value.toString();
  }

  private static void assertPeerRoundTrip(
      final IrohaPeerPayloadKind kind, final byte[] canonicalPayload) {
    final IrohaPeerWireMessageV1 encodedMessage =
        IrohaPeerKagemushaAdapterV1.wrap(kind, canonicalPayload);
    final byte[] encoded = encodedMessage.encode();
    assertEquals(kind.code(), encoded[8] & 0xff);
    final IrohaPeerWireMessageV1 decoded =
        IrohaPeerWireMessageV1.decode(
            encoded, IrohaPeerPayloadProfile.KAGEMUSHA_V1, kind);
    assertEquals(kind, decoded.canonicalPayload().kind());
    assertArrayEquals(canonicalPayload, IrohaPeerKagemushaAdapterV1.decode(decoded));
  }

  static String loadCanonicalFixture() throws Exception {
    return loadCanonicalFixture("fixtures/offline/kagemusha_v1.json");
  }

  private static String loadCanonicalFixture(final String relativePath) throws Exception {
    Path current = Paths.get("").toAbsolutePath().normalize();
    while (current != null) {
      final Path candidate = current.resolve(relativePath);
      if (Files.isRegularFile(candidate)) {
        return new String(Files.readAllBytes(candidate), StandardCharsets.UTF_8);
      }
      current = current.getParent();
    }
    throw new AssertionError(relativePath + " was not found");
  }

  static byte[] fixtureHex(final String fixture, final String section) {
    final String hex = fixtureString(fixture, section, "norito_hex");
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

  private static String fixtureString(
      final String fixture, final String section, final String field) {
    final String scope = fixtureScope(fixture, section);
    final Matcher fieldMatcher =
        Pattern.compile("\\\"" + Pattern.quote(field) + "\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"")
            .matcher(scope);
    if (!fieldMatcher.find()) {
      throw new AssertionError("fixture field " + field + " was not found");
    }
    return fieldMatcher.group(1);
  }

  private static long fixtureLong(
      final String fixture, final String section, final String field) {
    final String scope = fixtureScope(fixture, section);
    final Matcher fieldMatcher =
        Pattern.compile("\\\"" + Pattern.quote(field) + "\\\"\\s*:\\s*(\\d+)")
            .matcher(scope);
    if (!fieldMatcher.find()) {
      throw new AssertionError("fixture field " + field + " was not found");
    }
    return Long.parseLong(fieldMatcher.group(1));
  }

  private static boolean fixtureBoolean(
      final String fixture, final String section, final String field) {
    final Matcher fieldMatcher =
        Pattern.compile("\"" + Pattern.quote(field) + "\"\\s*:\\s*(true|false)")
            .matcher(fixtureScope(fixture, section));
    if (!fieldMatcher.find()) {
      throw new AssertionError("fixture field " + field + " was not found");
    }
    return Boolean.parseBoolean(fieldMatcher.group(1));
  }

  private static String fixtureScope(final String fixture, final String section) {
    if (section == null) {
      return fixture;
    }
    final Matcher sectionMatcher =
        Pattern.compile("\\\"" + Pattern.quote(section) + "\\\"\\s*:\\s*\\{")
            .matcher(fixture);
    if (!sectionMatcher.find()) {
      throw new AssertionError("fixture section " + section + " was not found");
    }
    final int objectStart = sectionMatcher.end();
    int depth = 1;
    boolean inString = false;
    boolean escaped = false;
    int objectEnd = objectStart;
    for (; objectEnd < fixture.length() && depth > 0; objectEnd++) {
      final char character = fixture.charAt(objectEnd);
      if (inString) {
        if (escaped) {
          escaped = false;
        } else if (character == '\\') {
          escaped = true;
        } else if (character == '"') {
          inString = false;
        }
      } else if (character == '"') {
        inString = true;
      } else if (character == '{') {
        depth++;
      } else if (character == '}') {
        depth--;
      }
    }
    if (depth != 0) {
      throw new AssertionError("fixture section " + section + " was not closed");
    }
    return fixture.substring(objectStart, objectEnd - 1);
  }

  private static List<String> fixtureMessageOrder(final String fixture) {
    final Matcher arrayMatcher =
        Pattern.compile(
                "\\\"ipm1_message_order\\\"\\s*:\\s*\\[(.*?)\\]", Pattern.DOTALL)
            .matcher(fixture);
    if (!arrayMatcher.find()) {
      throw new AssertionError("fixture ipm1_message_order was not found");
    }
    final Matcher entryMatcher =
        Pattern.compile(
                "\\{\\s*\\\"kind\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"\\s*,"
                    + "\\s*\\\"tag\\\"\\s*:\\s*(\\d+)\\s*\\}")
            .matcher(arrayMatcher.group(1));
    final List<String> result = new ArrayList<>();
    while (entryMatcher.find()) {
      result.add(entryMatcher.group(1) + ":" + entryMatcher.group(2));
    }
    return result;
  }

  private static String sha256Hex(final byte[] bytes) throws Exception {
    final byte[] digest = MessageDigest.getInstance("SHA-256").digest(bytes);
    final StringBuilder result = new StringBuilder(digest.length * 2);
    for (final byte value : digest) {
      result.append(String.format(Locale.ROOT, "%02x", value & 0xff));
    }
    return result.toString();
  }
}
