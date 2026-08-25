package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Base64;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;
import org.junit.Test;

/** Cross-SDK gate for the Rust-authored Offline Cash V1 semantics and shared IPM1/IQR1 vector. */
public final class OfflineCashInteropFixtureV1Tests {
  private static final String RUST_FIXTURE_SHA256 =
      "dc56c0852d926c9496c6f24e59e9143d28be5529e635473355bb2a8c696de257";
  private static final String TRANSPORT_FIXTURE_SHA256 =
      "f61c3f5be020dd99d034b89cc17f0e44e10ed8516e821caf109c3743a8f176b4";

  @Test
  public void rustFixturePinsSemanticProfile3MessagesAndExactKgm2Archives() throws Exception {
    final byte[] fixtureBytes = Files.readAllBytes(fixturePath("offline_cash_peer_transport_v1.json"));
    assertEquals(RUST_FIXTURE_SHA256, sha256Hex(fixtureBytes));
    final Map<String, Object> fixture = parse(fixtureBytes);
    assertEquals("iroha.offline-cash.peer-transport.v1", string(fixture, "schema"));
    assertEquals(22, integer(fixture, "native_bridge_abi"));

    final Map<String, Object> transport = object(fixture, "transport");
    assertEquals(3, integer(transport, "iroha_peer_wire_profile"));
    assertEquals(0x0100, integer(transport, "native_text_schema_version"));
    assertEquals(IrohaPeerWireLimitsV1.OFFLINE_CASH_TEXT_PREFIX, string(transport, "text_prefix"));

    final Map<String, Object> limits = object(fixture, "limits");
    assertEquals(
        IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_PAYMENT_REQUEST_RAW_BYTES,
        integer(limits, "payment_request_raw_max_bytes"));
    assertEquals(
        IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_PAYMENT_RAW_BYTES,
        integer(limits, "payment_raw_max_bytes"));
    assertEquals(
        IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_ACKNOWLEDGEMENT_RAW_BYTES,
        integer(limits, "acknowledgement_raw_max_bytes"));
    assertEquals(
        IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_PAYMENT_REQUEST_TEXT_BYTES,
        integer(limits, "payment_request_text_max_bytes"));
    assertEquals(
        IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_PAYMENT_TEXT_BYTES,
        integer(limits, "payment_text_max_bytes"));
    assertEquals(
        IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_ACKNOWLEDGEMENT_TEXT_BYTES,
        integer(limits, "acknowledgement_text_max_bytes"));
    assertEquals(
        IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_RAW_SESSION_BYTES,
        integer(limits, "raw_session_max_bytes"));
    assertEquals(
        IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_TEXT_SESSION_BYTES,
        integer(limits, "text_session_max_bytes"));
    assertEquals(
        IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_PAIRED_PROOF_BYTES,
        integer(limits, "paired_proof_max_bytes"));
    assertEquals(
        IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_PARITY_PROOF_BYTES,
        integer(limits, "parity_proof_max_bytes"));
    assertEquals(
        IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_ENCRYPTED_CREDIT_BYTES,
        integer(limits, "encrypted_credit_max_bytes"));

    final Map<String, Object> messages = object(fixture, "messages");
    assertMessage(
        object(messages, "payment_request"),
        IrohaPeerPayloadKind.RECEIVE_REQUEST,
        "receive_request",
        "receiver_payment_request",
        1,
        768,
        1_029,
        533,
        716);
    assertMessage(
        object(messages, "payment"),
        IrohaPeerPayloadKind.PAYMENT,
        "payment",
        "sender_payment",
        2,
        7_936,
        10_587,
        2_067,
        2_761);
    assertMessage(
        object(messages, "acknowledgement"),
        IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
        "acknowledgement",
        "receiver_acknowledgement_after_persist",
        3,
        256,
        347,
        249,
        337);

    final Map<String, Object> session = object(fixture, "session");
    assertEquals(2_849, integer(session, "raw_norito_bytes"));
    assertEquals(3_814, integer(session, "kgm2_text_bytes"));
  }

  @Test
  public void profile3PaymentMatchesSharedIpm1AndIqr1Vector() throws Exception {
    final byte[] fixtureBytes =
        Files.readAllBytes(fixturePath("offline_cash_profile3_ipm_iqr_v1.json"));
    assertEquals(TRANSPORT_FIXTURE_SHA256, sha256Hex(fixtureBytes));
    final Map<String, Object> fixture = parse(fixtureBytes);
    assertEquals("iroha.offline-cash.profile3-ipm-iqr.v1", string(fixture, "schema"));
    assertTrue(string(fixture, "source").contains("Rust does not generate IPM1 or IQR1"));

    final Map<String, Object> semanticFixture = object(fixture, "semantic_fixture");
    assertEquals(RUST_FIXTURE_SHA256, string(semanticFixture, "sha256_hex"));
    assertEquals("payment", string(semanticFixture, "message"));
    final Map<String, Object> payment =
        object(object(readFixture("offline_cash_peer_transport_v1.json"), "messages"), "payment");
    assertEquals(
        string(payment, "kgm2_text_sha256_hex"),
        string(semanticFixture, "kgm2_text_sha256_hex"));

    final byte[] canonicalText = string(payment, "kgm2_text").getBytes(StandardCharsets.UTF_8);
    final IrohaPeerCanonicalPayload payload =
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_CASH_V1,
            IrohaPeerPayloadKind.PAYMENT,
            0x0100,
            canonicalText);
    final IrohaPeerWireMessageV1 message =
        new IrohaPeerWireMessageV1(payload, IrohaPeerWireCompressionPolicyV1.PEER_OPTIMIZED);

    final Map<String, Object> transport = object(fixture, "transport");
    assertEquals(3, integer(transport, "profile"));
    assertEquals("payment", string(transport, "payload_kind"));
    assertEquals(2, integer(transport, "payload_kind_id"));
    assertEquals(0x0100, integer(transport, "schema_version"));
    assertEquals("peer_optimized", string(transport, "compression_policy"));
    assertEquals(IrohaPeerContentEncodingV1.ZLIB, message.encoding());
    assertEquals(
        string(transport, "selected_content_encoding"),
        message.encoding().name().toLowerCase(Locale.ROOT));
    assertEquals(integer(transport, "canonical_payload_bytes"), payload.byteCount());
    assertEquals(string(transport, "canonical_hash_hex"), hex(message.canonicalHash()));
    assertEquals(string(transport, "wire_hash_hex"), hex(message.wireHash()));
    assertEquals(string(transport, "stream_id_hex"), hex(message.streamId()));
    assertEquals(integer(transport, "encoded_body_bytes"), message.encodedBody().length);
    assertEquals(
        string(transport, "encoded_body_sha256_hex"), sha256Hex(message.encodedBody()));

    final byte[] encoded = message.encode();
    assertEquals(integer(transport, "ipm1_bytes"), encoded.length);
    assertEquals(string(transport, "ipm1_sha256_hex"), sha256Hex(encoded));
    assertEquals(string(transport, "ipm1_encoded_hex"), hex(encoded));
    assertEquals(
        message,
        IrohaPeerWireMessageV1.decode(
            encoded,
            IrohaPeerPayloadProfile.OFFLINE_CASH_V1,
            IrohaPeerPayloadKind.PAYMENT));

    final Map<String, Object> qr = object(fixture, "qr");
    assertNull(qr.get("static_complete_text"));
    assertNull(IrohaPeerQRCodecV1.staticCompleteTextCandidate(message));
    final List<String> texts = IrohaPeerQRCodecV1.animatedFrameTexts(message);
    assertEquals(integer(qr, "animated_frame_count"), texts.size());
    assertEquals(IrohaPeerQRCodecV1.PARITY_GROUP, integer(qr, "parity_group_width"));

    final List<Map<String, Object>> expectedFrames = objectList(qr, "frames");
    final List<IrohaPeerQRFrameV1> frames = new ArrayList<>();
    int dataFrames = 0;
    for (int index = 0; index < expectedFrames.size(); index++) {
      final Map<String, Object> expected = expectedFrames.get(index);
      final String text = texts.get(index);
      assertEquals(string(expected, "text"), text);
      final IrohaPeerQRFrameV1 frame = IrohaPeerQRCodecV1.decodeFrame(text);
      frames.add(frame);
      assertEquals(index, integer(expected, "sequence"));
      assertEquals(
          string(expected, "frame_kind"), frame.frameKind().name().toLowerCase(Locale.ROOT));
      assertEquals(integer(expected, "frame_kind_id"), frame.frameKind().code());
      assertEquals(IrohaPeerPayloadProfile.OFFLINE_CASH_V1, frame.profile());
      assertEquals(IrohaPeerPayloadKind.PAYMENT, frame.payloadKind());
      assertArrayEquals(message.streamId(), frame.streamId());
      assertEquals(integer(expected, "index"), frame.index());
      assertEquals(integer(expected, "total"), frame.total());
      assertEquals(string(expected, "payload_sha256_hex"), sha256Hex(frame.payload()));
      assertEquals(string(expected, "encoded_frame_sha256_hex"), sha256Hex(frame.encode()));
      assertEquals(
          string(expected, "text_sha256_hex"),
          sha256Hex(text.getBytes(StandardCharsets.UTF_8)));
      if (frame.frameKind() == IrohaPeerQRFrameV1.FrameKind.DATA) dataFrames++;
    }
    assertEquals(integer(qr, "data_frame_total"), dataFrames);
    assertParity(frames);
  }

  private static void assertMessage(
      final Map<String, Object> fixture,
      final IrohaPeerPayloadKind kind,
      final String peerKind,
      final String stage,
      final int kindId,
      final int rawMaximum,
      final int textMaximum,
      final int rawLength,
      final int textLength)
      throws Exception {
    assertEquals(Boolean.TRUE, fixture.get("semantic_valid"));
    assertEquals(3, integer(fixture, "iroha_peer_wire_profile"));
    assertEquals(0x0100, integer(fixture, "native_text_schema_version"));
    assertEquals(kindId, integer(fixture, "payload_kind_id"));
    assertEquals(kindId, kind.code());
    assertEquals(peerKind, string(fixture, "peer_payload_kind"));
    assertEquals(stage, string(fixture, "stage"));
    assertEquals(rawMaximum, integer(fixture, "maximum_raw_norito_bytes"));
    assertEquals(textMaximum, integer(fixture, "maximum_kgm2_text_bytes"));
    assertEquals(rawLength, integer(fixture, "raw_norito_bytes"));
    assertEquals(textLength, integer(fixture, "kgm2_text_bytes"));

    final byte[] raw = unhex(string(fixture, "raw_norito_hex"));
    final String text = string(fixture, "kgm2_text");
    final byte[] textBytes = text.getBytes(StandardCharsets.UTF_8);
    assertEquals(rawLength, raw.length);
    assertEquals(textLength, textBytes.length);
    assertEquals(
        IrohaPeerWireLimitsV1.OFFLINE_CASH_TEXT_PREFIX
            + Base64.getUrlEncoder().withoutPadding().encodeToString(raw),
        text);
    assertEquals(string(fixture, "raw_norito_sha256_hex"), sha256Hex(raw));
    assertEquals(string(fixture, "kgm2_text_sha256_hex"), sha256Hex(textBytes));

    final IrohaPeerCanonicalPayload payload =
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_CASH_V1, kind, 0x0100, textBytes);
    assertArrayEquals(textBytes, payload.bytes());
    final IrohaPeerWireMessageV1 message =
        new IrohaPeerWireMessageV1(payload, IrohaPeerWireCompressionPolicyV1.DISABLED);
    final IrohaPeerWireMessageV1 decoded =
        IrohaPeerWireMessageV1.decode(
            message.encode(), IrohaPeerPayloadProfile.OFFLINE_CASH_V1, kind);
    assertEquals(message, decoded);
    assertArrayEquals(textBytes, decoded.canonicalPayload().bytes());
  }

  private static void assertParity(final List<IrohaPeerQRFrameV1> frames) {
    final Map<Integer, byte[]> data = new HashMap<>();
    for (final IrohaPeerQRFrameV1 frame : frames) {
      if (frame.frameKind() == IrohaPeerQRFrameV1.FrameKind.DATA) {
        data.put(frame.index(), frame.payload());
      }
    }
    for (final IrohaPeerQRFrameV1 frame : frames) {
      if (frame.frameKind() != IrohaPeerQRFrameV1.FrameKind.PARITY) continue;
      final byte[] first = data.get(frame.index() * 2);
      final byte[] second = data.get(frame.index() * 2 + 1);
      final byte[] expected = first.clone();
      if (second != null) {
        for (int index = 0; index < expected.length; index++) expected[index] ^= second[index];
      }
      assertArrayEquals(expected, frame.payload());
    }
  }

  private static Map<String, Object> readFixture(final String name) throws Exception {
    return parse(Files.readAllBytes(fixturePath(name)));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> parse(final byte[] bytes) {
    return (Map<String, Object>) JsonParser.parse(new String(bytes, StandardCharsets.UTF_8));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Map<String, Object> value, final String name) {
    return (Map<String, Object>) value.get(name);
  }

  @SuppressWarnings("unchecked")
  private static List<Map<String, Object>> objectList(
      final Map<String, Object> value, final String name) {
    return (List<Map<String, Object>>) value.get(name);
  }

  private static String string(final Map<String, Object> value, final String name) {
    return (String) value.get(name);
  }

  private static int integer(final Map<String, Object> value, final String name) {
    return ((Number) value.get(name)).intValue();
  }

  private static Path fixturePath(final String name) {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve("fixtures/offline").resolve(name);
      if (Files.isRegularFile(candidate)) return candidate;
      current = current.getParent();
    }
    throw new AssertionError("fixtures/offline/" + name + " was not found");
  }

  private static String sha256Hex(final byte[] value) throws Exception {
    return hex(MessageDigest.getInstance("SHA-256").digest(value));
  }

  private static String hex(final byte[] value) {
    final StringBuilder out = new StringBuilder(value.length * 2);
    for (final byte element : value) out.append(String.format("%02x", element & 0xff));
    return out.toString();
  }

  private static byte[] unhex(final String value) {
    final byte[] out = new byte[value.length() / 2];
    for (int index = 0; index < out.length; index++) {
      out[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return out;
  }
}
