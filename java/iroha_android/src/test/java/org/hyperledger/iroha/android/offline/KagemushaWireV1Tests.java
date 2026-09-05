// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.hyperledger.iroha.sdk.offline.KagemushaAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentRequestV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentV1;
import org.junit.Test;

/** Java-mirror checks for the sole first-release KAGEMUSHA V1 wire surface. */
public final class KagemushaWireV1Tests {
  @Test
  public void sharedCanonicalFixtureRoundTripsByteForByte() throws Exception {
    final String fixture = loadCanonicalFixture();
    assertEquals(1L, fixtureLong(fixture, null, "fixture_version"));
    assertEquals("KAGEMUSHA", fixtureString(fixture, null, "protocol"));
    assertEquals("kgm1:", fixtureString(fixture, null, "text_prefix"));
    assertEquals(
        Arrays.asList("request:1", "payment:2", "acknowledgement:3"),
        fixtureMessageOrder(fixture));

    final String[] sections = {"payment_request", "payment", "acknowledgement"};
    final KagemushaWirePayloadKindV1[] textKinds = {
      KagemushaWirePayloadKindV1.PAYMENT_REQUEST,
      KagemushaWirePayloadKindV1.PAYMENT,
      KagemushaWirePayloadKindV1.ACKNOWLEDGEMENT
    };
    final IrohaPeerPayloadKind[] peerKinds = {
      IrohaPeerPayloadKind.REQUEST,
      IrohaPeerPayloadKind.PAYMENT,
      IrohaPeerPayloadKind.ACKNOWLEDGEMENT
    };
    final byte[][] rawSections = new byte[sections.length][];
    long rawBytes = 0;
    long textBytes = 0;
    for (int index = 0; index < sections.length; index++) {
      final byte[] raw = fixtureHex(fixture, sections[index]);
      final String text = fixtureString(fixture, sections[index], "kgm1");
      rawSections[index] = raw;
      rawBytes += raw.length;
      textBytes += text.length();
      assertEquals(index + 1L, fixtureLong(fixture, sections[index], "ipm1_kind"));
      assertEquals(raw.length, fixtureLong(fixture, sections[index], "raw_bytes"));
      assertEquals(fixtureString(fixture, sections[index], "sha256"), sha256Hex(raw));
      assertArrayEquals(raw, KagemushaWireV1.decodeText(textKinds[index], text));
      assertEquals(text, KagemushaWireV1.encodeText(textKinds[index], raw));
      assertPeerRoundTrip(peerKinds[index], raw);
    }
    assertEquals(rawBytes, fixtureLong(fixture, "complete_exchange", "raw_bytes"));
    assertEquals(textBytes, fixtureLong(fixture, "complete_exchange", "text_bytes"));

    final KagemushaPaymentRequestV1 request =
        KagemushaNoritoV1.decodePaymentRequestShapeExact(rawSections[0]);
    final KagemushaPaymentV1 payment =
        KagemushaNoritoV1.decodePaymentShapeExact(rawSections[1], request);
    final KagemushaAcknowledgementV1 acknowledgement =
        KagemushaNoritoV1.decodeAcknowledgementShapeExact(rawSections[2], request, payment);
    assertArrayEquals(rawSections[0], KagemushaNoritoV1.encodePaymentRequestShape(request));
    assertArrayEquals(rawSections[1], KagemushaNoritoV1.encodePaymentShape(payment, request));
    assertArrayEquals(
        rawSections[2],
        KagemushaNoritoV1.encodeAcknowledgementShape(acknowledgement, request, payment));
  }

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
  public void inventoryAndBoundsExactlyMirrorKotlin() {
    assertArrayEquals(
        new KagemushaWirePayloadKindV1[] {
          KagemushaWirePayloadKindV1.PAYMENT_REQUEST,
          KagemushaWirePayloadKindV1.PAYMENT,
          KagemushaWirePayloadKindV1.ACKNOWLEDGEMENT,
          KagemushaWirePayloadKindV1.MINT_AUTHORIZATION,
          KagemushaWirePayloadKindV1.MINT_CREDIT,
          KagemushaWirePayloadKindV1.REDEMPTION_VOUCHER,
        },
        KagemushaWirePayloadKindV1.values());
    assertArrayEquals(
        new IrohaPeerPayloadKind[] {
          IrohaPeerPayloadKind.REQUEST,
          IrohaPeerPayloadKind.PAYMENT,
          IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
        },
        IrohaPeerPayloadKind.values());
    assertEquals(1, IrohaPeerPayloadKind.REQUEST.code());
    assertEquals(2, IrohaPeerPayloadKind.PAYMENT.code());
    assertEquals(3, IrohaPeerPayloadKind.ACKNOWLEDGEMENT.code());
    assertEquals(928, KagemushaWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES);
    assertEquals(7_552, KagemushaWireV1.MAXIMUM_PAYMENT_BYTES);
    assertEquals(256, KagemushaWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES);
    assertEquals(9_211, KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES);
    assertEquals(12_288, KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES);
    for (final KagemushaWirePayloadKindV1 kind : KagemushaWirePayloadKindV1.values()) {
      assertEquals(kind.name(), kind.canonicalKind().name());
    }
  }

  @Test
  public void publicPaymentSurfaceContainsNoRetiredHandshake() {
    for (final Class<?> type :
        Arrays.asList(
            IrohaPeerPayloadKind.class,
            IrohaPeerNfcV1.class,
            KagemushaWirePayloadKindV1.class,
            KagemushaWireV1.class,
            KagemushaNoritoV1.class,
            KagemushaWalletV1.class)) {
      for (final Method method : type.getDeclaredMethods()) {
        assertNoRetiredHandshake(type, method.getName());
      }
      for (final Field field : type.getDeclaredFields()) {
        assertNoRetiredHandshake(type, field.getName());
      }
    }
  }

  private static void assertNoRetiredHandshake(final Class<?> owner, final String name) {
    final String lower = name.toLowerCase(Locale.ROOT);
    assertFalse(owner.getSimpleName() + " leaked acceptance API", lower.contains("acceptance"));
    assertFalse(owner.getSimpleName() + " leaked ticket API", lower.contains("ticket"));
    assertFalse(owner.getSimpleName() + " leaked request-mode API", lower.contains("requestmode"));
  }

  private static void assertPeerRoundTrip(
      final IrohaPeerPayloadKind kind, final byte[] canonicalPayload) {
    final IrohaPeerWireMessageV1 message = IrohaPeerKagemushaAdapterV1.wrap(kind, canonicalPayload);
    final byte[] encoded = message.encode();
    assertEquals(kind.code(), encoded[8] & 0xff);
    final IrohaPeerWireMessageV1 decoded =
        IrohaPeerWireMessageV1.decode(
            encoded, IrohaPeerPayloadProfile.KAGEMUSHA_V1, kind);
    assertArrayEquals(canonicalPayload, IrohaPeerKagemushaAdapterV1.decode(decoded));
  }

  private static String loadCanonicalFixture() throws Exception {
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

  private static byte[] fixtureHex(final String fixture, final String section) {
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
    final Matcher matcher =
        Pattern.compile("\\\"" + Pattern.quote(field) + "\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"")
            .matcher(fixtureScope(fixture, section));
    if (!matcher.find()) {
      throw new AssertionError("fixture field " + field + " was not found");
    }
    return matcher.group(1);
  }

  private static long fixtureLong(
      final String fixture, final String section, final String field) {
    final Matcher matcher =
        Pattern.compile("\\\"" + Pattern.quote(field) + "\\\"\\s*:\\s*(\\d+)")
            .matcher(fixtureScope(fixture, section));
    if (!matcher.find()) {
      throw new AssertionError("fixture field " + field + " was not found");
    }
    return Long.parseLong(matcher.group(1));
  }

  private static String fixtureScope(final String fixture, final String section) {
    if (section == null) {
      return fixture;
    }
    final Matcher sectionMatcher =
        Pattern.compile("\\\"" + Pattern.quote(section) + "\\\"\\s*:\\s*\\{").matcher(fixture);
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
        Pattern.compile("\\\"ipm1_message_order\\\"\\s*:\\s*\\[(.*?)\\]", Pattern.DOTALL)
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
