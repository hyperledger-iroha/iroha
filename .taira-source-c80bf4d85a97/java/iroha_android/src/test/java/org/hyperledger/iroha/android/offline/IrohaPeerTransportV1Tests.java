package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.zip.Deflater;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyAuthenticationV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyEncryptedRecordV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyP256V1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbySessionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbySignatureVerifierV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcAPDUCodecV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverSessionV1;
import org.junit.Test;

public final class IrohaPeerTransportV1Tests {
  @Test
  public void wireLimitsCannotExceedV1HardCeilings() {
    new IrohaPeerWireLimitsV1(32 * 1_024, 24_576, 24_576);
    assertThrows(IllegalArgumentException.class,
        () -> new IrohaPeerWireLimitsV1(32 * 1_024 + 1, 24_576, 24_576));
    assertThrows(IllegalArgumentException.class,
        () -> new IrohaPeerWireLimitsV1(32 * 1_024, 24_577, 24_576));
    assertThrows(IllegalArgumentException.class,
        () -> new IrohaPeerWireLimitsV1(32 * 1_024, 24_576, 24_577));
  }

  @Test
  public void canonicalTextCodecPreservesOfflineNoteExactUtf8() {
    final String signedWalletText = "pk2off2:eyJsaW5lYWdlIjoiY2Fub25pY2FsIn0.署名";
    final byte[] encoded = IrohaPeerCanonicalTextPayloadCodecV1.canonicalBytes(
        signedWalletText, IrohaPeerPayloadProfile.OFFLINE_NOTE);
    assertArrayEquals(signedWalletText.getBytes(StandardCharsets.UTF_8), encoded);
    assertEquals(
        signedWalletText,
        IrohaPeerCanonicalTextPayloadCodecV1.canonicalText(
            encoded, IrohaPeerPayloadProfile.OFFLINE_NOTE));
    assertEquals(
        24_576,
        IrohaPeerCanonicalTextPayloadCodecV1.maximumCanonicalTextBytes(
            IrohaPeerPayloadProfile.OFFLINE_NOTE));
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerCanonicalTextPayloadCodecV1.maximumCanonicalTextBytes(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND));

    final IrohaPeerWireLimitsV1 canonicalLimited = new IrohaPeerWireLimitsV1(7, 9, 8);
    assertEquals(
        7,
        IrohaPeerCanonicalTextPayloadCodecV1.maximumCanonicalTextBytes(
            IrohaPeerPayloadProfile.OFFLINE_NOTE, canonicalLimited));
  }

  @Test
  public void canonicalTextCodecRejectsEmptyInvalidUtf8AndProfileOversizeInput() {
    final IrohaPeerPayloadProfile profile = IrohaPeerPayloadProfile.OFFLINE_NOTE;
    {
      assertThrows(
          IllegalArgumentException.class,
          () -> IrohaPeerCanonicalTextPayloadCodecV1.canonicalBytes("", profile));
      assertThrows(
          IllegalArgumentException.class,
          () -> IrohaPeerCanonicalTextPayloadCodecV1.canonicalText(new byte[0], profile));
      assertThrows(
          IllegalArgumentException.class,
          () -> IrohaPeerCanonicalTextPayloadCodecV1.canonicalText(
              new byte[] {(byte) 0xc3, 0x28}, profile));
      final int maximum =
          IrohaPeerCanonicalTextPayloadCodecV1.maximumCanonicalTextBytes(profile);
      assertThrows(
          IllegalArgumentException.class,
          () -> IrohaPeerCanonicalTextPayloadCodecV1.canonicalBytes(
              "a".repeat(maximum + 1), profile));
      final byte[] oversized = new byte[maximum + 1];
      Arrays.fill(oversized, (byte) 'a');
      assertThrows(
          IllegalArgumentException.class,
          () -> IrohaPeerCanonicalTextPayloadCodecV1.canonicalText(oversized, profile));
    }

    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerCanonicalTextPayloadCodecV1.canonicalBytes(
            "not-a-native-archive", IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND));
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerCanonicalTextPayloadCodecV1.canonicalText(
            "not-a-native-archive".getBytes(StandardCharsets.UTF_8),
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND));

    // Java String can contain an unpaired surrogate, but it is not exact UTF-8.
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerCanonicalTextPayloadCodecV1.canonicalBytes(
            "\uD800", IrohaPeerPayloadProfile.OFFLINE_NOTE));
  }

  @Test
  public void rejectsEmptyCanonicalIpm1Payloads() {
    assertThrows(
        IllegalArgumentException.class,
        () -> new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerPayloadKind.PAYMENT,
            1,
            new byte[0]));
  }

  @Test
  public void kagemushaProfileRequiresExactNativeIndependentAbi21Envelope() {
    final byte[] canonical =
        kagemushaArchive(IrohaPeerPayloadKind.RECEIVE_REQUEST, new byte[] {0x51});
    assertEquals(49, canonical.length);
    assertEquals(
        "4e5254300000bfd427e87daf1d5cfa39b7fb60a76859000100000000000000"
            + "de8130dd3f67aeb502000000000000000051",
        hex(canonical));
    final IrohaPeerWireMessageV1 message = new IrohaPeerWireMessageV1(
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerPayloadKind.RECEIVE_REQUEST,
            0x0102,
            canonical));
    assertEquals(message, IrohaPeerWireMessageV1.decode(message.encode()));

    final byte[] wrongSchema = canonical.clone();
    wrongSchema[6] ^= 1;
    final byte[] shortPadding = concat(
        Arrays.copyOfRange(canonical, 0, 40),
        Arrays.copyOfRange(canonical, 41, canonical.length));
    final byte[] longPadding = concat(
        Arrays.copyOfRange(canonical, 0, 40),
        new byte[] {0},
        Arrays.copyOfRange(canonical, 40, canonical.length));
    final byte[] wrongChecksum = canonical.clone();
    wrongChecksum[31] ^= 1;
    final byte[] wrongFlags = canonical.clone();
    wrongFlags[39] = 0;
    final byte[] wrongCompression = canonical.clone();
    wrongCompression[22] = 1;
    final byte[] trailing = concat(canonical, new byte[] {0});
    final byte[] body = {0x51};
    final NoritoHeader bareHeader = new NoritoHeader(
        SchemaHash.hash16("OfflineRecipientReceiveOfferV2"),
        body.length,
        CRC64.compute(body),
        NoritoHeader.COMPACT_LEN,
        NoritoHeader.COMPRESSION_NONE);
    final byte[] bareSchema = concat(bareHeader.encode(), new byte[8], body);

    for (final byte[] invalid : List.of(
        wrongSchema,
        shortPadding,
        longPadding,
        wrongChecksum,
        wrongFlags,
        wrongCompression,
        trailing,
        bareSchema)) {
      assertThrows(
          IllegalArgumentException.class,
          () -> new IrohaPeerCanonicalPayload(
              IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
              IrohaPeerPayloadKind.RECEIVE_REQUEST,
              0x0102,
              invalid));
    }
    assertThrows(
        IllegalArgumentException.class,
        () -> new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerPayloadKind.PAYMENT,
            0x0102,
            canonical));
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerWireMessageV1.decode(
            rehashKagemushaMessage(message.encode(), wrongSchema)));
  }

  @Test
  @SuppressWarnings("unchecked")
  public void qualifiedKagemushaStructuralFixtureCrossesIpmQrNfcAndNearbyByteForByte()
      throws Exception {
    final Map<String, Object> fixture = (Map<String, Object>) JsonParser.parse(
        new String(Files.readAllBytes(sharedKagemushaFixture()), StandardCharsets.UTF_8));
    assertEquals(Boolean.FALSE, fixture.get("semantic_valid"));
    final Map<String, Object> norito = (Map<String, Object>) fixture.get("norito");
    final byte[] archive = unhex((String) norito.get("archive_hex"));
    assertEquals(49, archive.length);

    final IrohaPeerWireMessageV1 message = new IrohaPeerWireMessageV1(
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerPayloadKind.RECEIVE_REQUEST,
            0x0102,
            archive));
    final Map<String, Object> ipm = (Map<String, Object>) fixture.get("ipm1");
    assertEquals(((Number) ipm.get("message_bytes")).intValue(), message.encode().length);
    assertEquals(ipm.get("canonical_hash_hex"), hex(message.canonicalHash()));
    assertEquals(ipm.get("wire_hash_hex"), hex(message.wireHash()));
    assertArrayEquals(unhex((String) ipm.get("encoded_hex")), message.encode());
    assertEquals(message, IrohaPeerWireMessageV1.decode(message.encode()));

    final Map<String, Object> qr = (Map<String, Object>) fixture.get("qr");
    final List<String> frames = IrohaPeerQRCodecV1.encode(message);
    assertEquals(((Number) qr.get("frame_count")).intValue(), frames.size());
    assertEquals(qr.get("static_text"), frames.get(0));
    assertEquals(message, new IrohaPeerQRScanSessionV1().ingest(frames.get(0)).message());

    final Map<String, Object> nfc = (Map<String, Object>) fixture.get("nfc");
    final byte[] nfcSession = unhex((String) nfc.get("session_hex"));
    final IrohaPeerNfcReceiverSessionV1 receiverCard =
        new IrohaPeerNfcReceiverSessionV1(nfcSession, message.encode());
    assertArrayEquals(unhex((String) nfc.get("info_hex")), receiverCard.info().encode());
    final IrohaPeerNfcCommandV1 read = IrohaPeerNfcCommandV1.readRequest(
        nfcSession,
        message.canonicalHash(),
        0,
        message.encode().length);
    assertArrayEquals(
        unhex((String) nfc.get("read_request_apdu_hex")),
        IrohaPeerNfcAPDUCodecV1.encode(read));
    assertArrayEquals(
        unhex((String) nfc.get("read_request_response_hex")),
        receiverCard.handle(read));

    final Map<String, Object> nearby = (Map<String, Object>) fixture.get("nearby");
    final byte[] nearbySession = unhex((String) nearby.get("session_hex"));
    final byte[] requestHash = unhex((String) nearby.get("request_hash_hex"));
    assertArrayEquals(message.canonicalHash(), requestHash);
    final IrohaPeerNearbySessionV1 sender = new IrohaPeerNearbySessionV1(
        org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
        org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
        nearbySession,
        requestHash,
        unhex((String) nearby.get("sender_certificate_hex")),
        repeat(32, ((Number) nearby.get("sender_nonce_repeat_byte")).intValue()),
        IrohaPeerNearbyP256V1.fromPrivateBytes(
            scalar(((Number) nearby.get("sender_private_scalar")).intValue())));
    final IrohaPeerNearbySessionV1 nearbyReceiver = new IrohaPeerNearbySessionV1(
        org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
        org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.RECEIVER,
        nearbySession,
        requestHash,
        unhex((String) nearby.get("receiver_certificate_hex")),
        repeat(32, ((Number) nearby.get("receiver_nonce_repeat_byte")).intValue()),
        IrohaPeerNearbyP256V1.fromPrivateBytes(
            scalar(((Number) nearby.get("receiver_private_scalar")).intValue())));
    sender.acceptPeerHello(nearbyReceiver.getLocalHello());
    nearbyReceiver.acceptPeerHello(sender.getLocalHello());
    final IrohaPeerNearbyAuthenticationV1 senderAuthentication = sender.makeAuthentication(
        unhex((String) nearby.get("sender_authentication_signature_hex")));
    final IrohaPeerNearbyAuthenticationV1 receiverAuthentication =
        nearbyReceiver.makeAuthentication(
            unhex((String) nearby.get("receiver_authentication_signature_hex")));
    assertEquals(
        nearby.get("transcript_hash_hex"),
        hex(senderAuthentication.getTranscriptHash()));
    final IrohaPeerNearbySignatureVerifierV1 acceptAll = (role, cert, signed, signature) -> true;
    sender.acceptPeerAuthentication(receiverAuthentication, acceptAll);
    nearbyReceiver.acceptPeerAuthentication(senderAuthentication, acceptAll);
    final IrohaPeerNearbyEncryptedRecordV1 record = sender.seal(message.encode());
    assertEquals(nearby.get("sender_record_hex"), hex(record.encode()));
    assertArrayEquals(
        message.encode(),
        nearbyReceiver.open(IrohaPeerNearbyEncryptedRecordV1.decode(record.encode())));
  }

  @Test
  @SuppressWarnings("unchecked")
  public void matchesSharedIpm1AndIqr1GoldenVector() throws Exception {
    final String fixtureText =
        new String(Files.readAllBytes(sharedFixture()), StandardCharsets.UTF_8);
    final Map<String, Object> fixture = (Map<String, Object>) JsonParser.parse(fixtureText);
    final IrohaPeerCanonicalPayload payload =
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerPayloadKind.PAYMENT,
            1,
            ((String) fixture.get("canonical_utf8")).getBytes(StandardCharsets.UTF_8));
    final IrohaPeerWireMessageV1 message = new IrohaPeerWireMessageV1(payload);
    assertEquals(fixture.get("ipm1_hex"), hex(message.encode()));
    assertEquals(fixture.get("canonical_hash_hex"), hex(message.canonicalHash()));
    assertEquals(fixture.get("wire_hash_hex"), hex(message.wireHash()));
    assertEquals(List.of(fixture.get("iqr1")), IrohaPeerQRCodecV1.encode(message));
    assertEquals(
        message,
        IrohaPeerWireMessageV1.decode(
            message.encode(), IrohaPeerPayloadProfile.OFFLINE_NOTE, IrohaPeerPayloadKind.PAYMENT));
    final String canonicalQr = IrohaPeerQRCodecV1.encode(message).get(0);
    assertEquals(message, new IrohaPeerQRScanSessionV1().ingest(canonicalQr).message());
    for (final String nonCanonical :
        List.of(" " + canonicalQr, canonicalQr + "\t", "\n" + canonicalQr)) {
      assertThrows(
          IllegalArgumentException.class,
          () -> new IrohaPeerQRScanSessionV1().ingest(nonCanonical));
    }

    final Map<String, Object> zlib = (Map<String, Object>) fixture.get("zlib");
    final byte[] compressedBytes = unhex((String) zlib.get("ipm1_hex"));
    final IrohaPeerWireMessageV1 compressed = IrohaPeerWireMessageV1.decode(compressedBytes);
    assertEquals(IrohaPeerContentEncodingV1.ZLIB, compressed.encoding());
    final byte[] repeated = new byte[1024];
    Arrays.fill(repeated, (byte) 65);
    assertArrayEquals(repeated, compressed.canonicalPayload().bytes());
    assertEquals(
        zlib.get("iqr1"),
        IrohaPeerQRCodecV1.encodeFrame(
            new IrohaPeerQRFrameV1(
                IrohaPeerQRFrameV1.FrameKind.COMPLETE,
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerPayloadKind.PAYMENT,
                compressed.streamId(),
                0,
                1,
                compressedBytes)));
  }

  @Test
  @SuppressWarnings("unchecked")
  public void matchesSharedAnimatedIrqrFrameBytesAndOrder() throws Exception {
    final Map<String, Object> fixture = (Map<String, Object>) JsonParser.parse(
        new String(Files.readAllBytes(sharedFixture()), StandardCharsets.UTF_8));
    final Map<String, Object> animated = (Map<String, Object>) fixture.get("animated");
    int state = ((Number) animated.get("canonical_seed")).intValue();
    final byte[] canonical = new byte[((Number) animated.get("canonical_count")).intValue()];
    for (int index = 0; index < canonical.length; index++) {
      state = state * 1_664_525 + 1_013_904_223;
      canonical[index] = (byte) (state >>> 24);
    }
    final IrohaPeerWireMessageV1 message = new IrohaPeerWireMessageV1(
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerPayloadKind.PAYMENT,
            1,
            canonical));
    assertEquals(animated.get("wire_hash_hex"), hex(message.wireHash()));
    final List<String> descriptors = new ArrayList<>();
    final List<String> frameDigests = new ArrayList<>();
    for (final String text : IrohaPeerQRCodecV1.animatedFrameTexts(message)) {
      final IrohaPeerQRFrameV1 frame = IrohaPeerQRCodecV1.decodeFrame(text);
      descriptors.add(frame.frameKind().code() + ":" + frame.index() + ":" + frame.total());
      frameDigests.add(hex(Blake2b.digest256(frame.encode())));
    }
    assertEquals(animated.get("frame_kind_index_total"), descriptors);
    assertEquals(animated.get("frame_blake2b_256_hex"), frameDigests);
  }

  @Test
  public void recoversOneMissingAnimatedShardWithHeaderLast() {
    final byte[] payload = new byte[652];
    for (int index = 0; index < payload.length; index++) payload[index] = (byte) index;
    final byte[] canonical = kagemushaArchive(IrohaPeerPayloadKind.PAYMENT, payload);
    final IrohaPeerWireMessageV1 message =
        new IrohaPeerWireMessageV1(
            new IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                IrohaPeerPayloadKind.PAYMENT,
                0x0102,
                canonical));
    final List<String> texts = IrohaPeerQRCodecV1.encode(message);
    final List<IrohaPeerQRFrameV1> frames = new ArrayList<>();
    for (final String text : texts) frames.add(IrohaPeerQRCodecV1.decodeFrame(text));
    assertEquals(
        List.of(
            IrohaPeerQRFrameV1.FrameKind.HEADER,
            IrohaPeerQRFrameV1.FrameKind.DATA,
            IrohaPeerQRFrameV1.FrameKind.DATA,
            IrohaPeerQRFrameV1.FrameKind.PARITY,
            IrohaPeerQRFrameV1.FrameKind.DATA,
            IrohaPeerQRFrameV1.FrameKind.PARITY),
        frames.stream().map(IrohaPeerQRFrameV1::frameKind).toList());
    assertTrue(texts.stream().allMatch(value -> value.getBytes(StandardCharsets.UTF_8).length <= 700));

    final IrohaPeerQRScanSessionV1 session = new IrohaPeerQRScanSessionV1();
    session.ingest(textFor(frames, texts, IrohaPeerQRFrameV1.FrameKind.DATA, 1));
    session.ingest(textFor(frames, texts, IrohaPeerQRFrameV1.FrameKind.PARITY, 0));
    session.ingest(textFor(frames, texts, IrohaPeerQRFrameV1.FrameKind.DATA, 2));
    final IrohaPeerQRScanResultV1 result = session.ingest(texts.get(0));
    assertEquals(message, result.message());
    assertEquals(1, result.recoveredDataFrames());
  }

  @Test
  public void completedScannerRollsOverAndTamperingFailsClosed() {
    final IrohaPeerWireMessageV1 first = message("first");
    final IrohaPeerWireMessageV1 second = message("second");
    final IrohaPeerQRScanSessionV1 session = new IrohaPeerQRScanSessionV1();
    assertEquals(first, session.ingest(IrohaPeerQRCodecV1.encode(first).get(0)).message());
    assertEquals(second, session.ingest(IrohaPeerQRCodecV1.encode(second).get(0)).message());

    final byte[] tampered = first.encode();
    tampered[tampered.length - 1] ^= 1;
    assertThrows(IllegalArgumentException.class, () -> IrohaPeerWireMessageV1.decode(tampered));
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerWireMessageV1.decode(
            first.encode(), IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND, null));
    final byte[] corruptFrame =
        IrohaPeerQRCodecV1.decodeFrame(IrohaPeerQRCodecV1.encode(first).get(0)).encode();
    corruptFrame[corruptFrame.length - 1] ^= 1;
    assertThrows(IllegalArgumentException.class, () -> IrohaPeerQRFrameV1.decode(corruptFrame));
  }

  @Test
  public void enforcesFirstReleaseProfileSchemasAndRetailTwentyFourKiBBodyBound() {
    assertEquals(24_576, IrohaPeerWireMessageV1.MAXIMUM_OFFLINE_NOTE_ENCODED_BYTES);
    assertEquals(24_576, IrohaPeerWireLimitsV1.PEER_V1.maximumOfflineNoteEncodedBytes());
    assertEquals(24_576, IrohaPeerWireMessageV1.MAXIMUM_KAGEMUSHA_ENCODED_BYTES);

    final byte[] boundaryBytes = new byte[24_576];
    Arrays.fill(boundaryBytes, (byte) 0x5a);
    final IrohaPeerWireMessageV1 boundary =
        new IrohaPeerWireMessageV1(
            new IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerPayloadKind.PAYMENT,
                1,
                boundaryBytes));
    assertEquals(24_576, boundary.encodedBody().length);
    assertEquals(boundary, IrohaPeerWireMessageV1.decode(boundary.encode()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new IrohaPeerWireMessageV1(
                new IrohaPeerCanonicalPayload(
                    IrohaPeerPayloadProfile.OFFLINE_NOTE,
                    IrohaPeerPayloadKind.PAYMENT,
                    1,
                    new byte[24_577])));

    final IllegalArgumentException retailMismatch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new IrohaPeerCanonicalPayload(
                    IrohaPeerPayloadProfile.OFFLINE_NOTE,
                    IrohaPeerPayloadKind.PAYMENT,
                    0x0102,
                    new byte[] {1}));
    assertTrue(retailMismatch.getMessage().contains("requires schema 1, received 258"));
    final IllegalArgumentException kagemushaMismatch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new IrohaPeerCanonicalPayload(
                    IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                    IrohaPeerPayloadKind.PAYMENT,
                    1,
                    new byte[] {1}));
    assertTrue(kagemushaMismatch.getMessage().contains("requires schema 258, received 1"));

    final byte[] hostileHeader = Arrays.copyOf(boundary.encode(), IrohaPeerWireMessageV1.HEADER_LENGTH);
    hostileHeader[10] = 0x01;
    hostileHeader[11] = 0x02;
    final IllegalArgumentException hostile =
        assertThrows(
            IllegalArgumentException.class,
            () -> IrohaPeerWireMessageV1.decodeHeader(hostileHeader));
    assertTrue(hostile.getMessage().contains("requires schema 1, received 258"));
  }

  @Test
  public void decodesOnlyCanonicallyUsefulZlib() {
    final byte[] canonical = new byte[1024];
    Arrays.fill(canonical, (byte) 0x41);
    final IrohaPeerWireMessageV1 decoded = IrohaPeerWireMessageV1.decode(zlibMessage(canonical));
    assertEquals(IrohaPeerContentEncodingV1.ZLIB, decoded.encoding());
    assertArrayEquals(canonical, decoded.canonicalPayload().bytes());
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerWireMessageV1.decode(zlibMessage(canonical, new byte[] {0})));

    final byte[] small = new byte[100];
    Arrays.fill(small, (byte) 0x41);
    assertThrows(IllegalArgumentException.class, () -> IrohaPeerWireMessageV1.decode(zlibMessage(small)));
  }

  private static IrohaPeerWireMessageV1 message(final String text) {
    return new IrohaPeerWireMessageV1(
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerPayloadKind.PAYMENT,
            1,
            text.getBytes(StandardCharsets.UTF_8)));
  }

  private static String textFor(
      final List<IrohaPeerQRFrameV1> frames,
      final List<String> texts,
      final IrohaPeerQRFrameV1.FrameKind kind,
      final int index) {
    for (int position = 0; position < frames.size(); position++) {
      final IrohaPeerQRFrameV1 frame = frames.get(position);
      if (frame.frameKind() == kind && frame.index() == index) return texts.get(position);
    }
    throw new AssertionError("frame not found");
  }

  private static byte[] zlibMessage(final byte[] canonical) {
    return zlibMessage(canonical, new byte[0]);
  }

  private static byte[] kagemushaArchive(
      final IrohaPeerPayloadKind kind, final byte[] payload) {
    final String schema = switch (kind) {
      case RECEIVE_REQUEST ->
          "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2";
      case PAYMENT ->
          "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4";
      case ACKNOWLEDGEMENT ->
          "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2";
    };
    final int padding = kind == IrohaPeerPayloadKind.ACKNOWLEDGEMENT ? 0 : 8;
    final NoritoHeader header = new NoritoHeader(
        SchemaHash.hash16(schema),
        payload.length,
        CRC64.compute(payload),
        NoritoHeader.COMPACT_LEN,
        NoritoHeader.COMPRESSION_NONE);
    return concat(header.encode(), new byte[padding], payload);
  }

  private static byte[] rehashKagemushaMessage(
      final byte[] encoded, final byte[] canonical) {
    if (encoded.length != IrohaPeerWireMessageV1.HEADER_LENGTH + canonical.length) {
      throw new IllegalArgumentException("Kagemusha fixture length mismatch");
    }
    final byte[] result = encoded.clone();
    System.arraycopy(canonical, 0, result, IrohaPeerWireMessageV1.HEADER_LENGTH, canonical.length);
    final byte[] canonicalHash = Blake2b.digest256(concat(
        "IROHA-PEER-PAYLOAD-V1\0".getBytes(StandardCharsets.UTF_8),
        new byte[] {0, 2, 1, 1, 2},
        canonical));
    System.arraycopy(canonicalHash, 0, result, 20, canonicalHash.length);
    final byte[] wireHash = Blake2b.digest256(concat(
        "IROHA-PEER-MESSAGE-V1\0".getBytes(StandardCharsets.UTF_8),
        Arrays.copyOfRange(result, 0, 52),
        canonical));
    System.arraycopy(wireHash, 0, result, 52, wireHash.length);
    return result;
  }

  private static byte[] zlibMessage(final byte[] canonical, final byte[] trailing) {
    final Deflater deflater = new Deflater(Deflater.DEFAULT_COMPRESSION, false);
    final ByteArrayOutputStream output = new ByteArrayOutputStream();
    final byte[] buffer = new byte[256];
    try {
      deflater.setInput(canonical);
      deflater.finish();
      while (!deflater.finished()) output.write(buffer, 0, deflater.deflate(buffer));
    } finally {
      deflater.end();
    }
    final byte[] body = concat(output.toByteArray(), trailing);
    final byte[] metadata = {0, 1, 2, 0, 1};
    final byte[] canonicalHash =
        Blake2b.digest256(
            concat(
                "IROHA-PEER-PAYLOAD-V1\0".getBytes(StandardCharsets.UTF_8),
                metadata,
                canonical));
    final byte[] prefix = new byte[52];
    System.arraycopy("IPM1".getBytes(StandardCharsets.US_ASCII), 0, prefix, 0, 4);
    prefix[4] = 1;
    prefix[5] = 1;
    prefix[6] = 0;
    prefix[7] = 1;
    prefix[8] = 2;
    prefix[9] = 0;
    prefix[10] = 0;
    prefix[11] = 1;
    IrohaPeerWireMessageV1.writeU32(prefix, 12, canonical.length);
    IrohaPeerWireMessageV1.writeU32(prefix, 16, body.length);
    System.arraycopy(canonicalHash, 0, prefix, 20, canonicalHash.length);
    final byte[] wireHash =
        Blake2b.digest256(
            concat(
                "IROHA-PEER-MESSAGE-V1\0".getBytes(StandardCharsets.UTF_8),
                prefix,
                body));
    return concat(prefix, wireHash, body);
  }

  private static byte[] concat(final byte[]... values) {
    int length = 0;
    for (final byte[] value : values) length += value.length;
    final byte[] out = new byte[length];
    int offset = 0;
    for (final byte[] value : values) {
      System.arraycopy(value, 0, out, offset, value.length);
      offset += value.length;
    }
    return out;
  }

  private static Path sharedFixture() {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve("fixtures/offline/peer_transport_v1.json");
      if (Files.isRegularFile(candidate)) return candidate;
      current = current.getParent();
    }
    throw new AssertionError("peer_transport_v1.json was not found");
  }

  private static Path sharedKagemushaFixture() {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve("fixtures/offline/kagemusha_peer_transport_v2.json");
      if (Files.isRegularFile(candidate)) return candidate;
      current = current.getParent();
    }
    throw new AssertionError("kagemusha_peer_transport_v2.json was not found");
  }

  private static byte[] scalar(final int value) {
    final byte[] bytes = new byte[32];
    bytes[31] = (byte) value;
    return bytes;
  }

  private static byte[] repeat(final int count, final int value) {
    final byte[] bytes = new byte[count];
    Arrays.fill(bytes, (byte) value);
    return bytes;
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
