package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.junit.Test;

public final class IrohaPeerQRScanSessionV1Tests {
  @Test
  public void scanLimitsCannotExceedV1HardCeilings() {
    new IrohaPeerQRScanLimitsV1(3, 12, 3_072, 30_000, 180_000);
    assertThrows(IllegalArgumentException.class,
        () -> new IrohaPeerQRScanLimitsV1(4, 12, 3_072, 30_000, 180_000));
    assertThrows(IllegalArgumentException.class,
        () -> new IrohaPeerQRScanLimitsV1(3, 13, 3_072, 30_000, 180_000));
    assertThrows(IllegalArgumentException.class,
        () -> new IrohaPeerQRScanLimitsV1(3, 12, 3_073, 30_000, 180_000));
    assertThrows(IllegalArgumentException.class,
        () -> new IrohaPeerQRScanLimitsV1(3, 12, 3_072, 30_001, 180_000));
    assertThrows(IllegalArgumentException.class,
        () -> new IrohaPeerQRScanLimitsV1(3, 12, 3_072, 30_000, 180_001));
  }

  @Test
  public void keepsThreeInterleavedStreamsAndRejectsFourthWithoutEviction() {
    final List<IrohaPeerWireMessageV1> messages = new ArrayList<>();
    final List<List<String>> frames = new ArrayList<>();
    for (int seed = 1; seed <= 4; seed++) {
      final IrohaPeerWireMessageV1 message = animatedMessage(seed);
      messages.add(message);
      frames.add(IrohaPeerQRCodecV1.animatedFrameTexts(message));
    }
    final IrohaPeerQRScanSessionV1 session = new IrohaPeerQRScanSessionV1();
    for (int index = 0; index < 3; index++) session.ingestAt(frames.get(index).get(0), 1_000);
    assertEquals(3, session.activeStreamCount());
    assertThrows(IllegalArgumentException.class, () -> session.ingestAt(frames.get(3).get(0), 1_000));
    assertEquals(3, session.activeStreamCount());

    IrohaPeerWireMessageV1 completed = null;
    for (final String text : frames.get(0).subList(1, frames.get(0).size())) {
      final IrohaPeerQRScanResultV1 result = session.ingestAt(text, 1_001);
      if (result.message() != null) {
        completed = result.message();
        break;
      }
    }
    assertEquals(messages.get(0), completed);
    assertEquals(2, session.activeStreamCount());
  }

  @Test
  public void applicationRejectionCanQuarantineCompletedStreamWithBoundedLifetime() {
    final IrohaPeerQRScanLimitsV1 limits =
        new IrohaPeerQRScanLimitsV1(3, 12, 3_072, 10, 30);
    final IrohaPeerWireMessageV1 message = messageWithBytes(new byte[] {9, 1, 7});
    final String text = IrohaPeerQRCodecV1.encode(message).get(0);
    final IrohaPeerQRScanSessionV1 session = new IrohaPeerQRScanSessionV1(limits);
    final IrohaPeerWireMessageV1 completed = session.ingestAt(text, 1_000).message();
    assertEquals(message, completed);
    assertEquals(0, session.activeStreamCount());

    session.quarantine(completed.streamId(), 1_000);
    final IllegalArgumentException quarantined =
        assertThrows(IllegalArgumentException.class, () -> session.ingestAt(text, 1_029));
    assertTrue(quarantined.getMessage().contains("quarantined"));
    assertEquals(message, session.ingestAt(text, 1_030).message());
    assertThrows(
        IllegalArgumentException.class,
        () -> session.quarantine(new byte[15], 1_031));

    final List<IrohaPeerWireMessageV1> bounded = new ArrayList<>();
    for (int index = 0; index < 13; index++) {
      final IrohaPeerWireMessageV1 item = messageWithBytes(new byte[] {(byte) index});
      bounded.add(item);
      session.quarantine(item.streamId(), 2_000L + index);
    }
    assertEquals(
        bounded.get(0),
        session.ingestAt(IrohaPeerQRCodecV1.encode(bounded.get(0)).get(0), 2_013).message());
    final IllegalArgumentException retained =
        assertThrows(
            IllegalArgumentException.class,
            () -> session.ingestAt(
                IrohaPeerQRCodecV1.encode(bounded.get(1)).get(0), 2_013));
    assertTrue(retained.getMessage().contains("quarantined"));

    final IrohaPeerQRScanSessionV1 saturated = new IrohaPeerQRScanSessionV1(limits);
    saturated.quarantine(message.streamId(), Long.MAX_VALUE);
    final IllegalArgumentException atMaximum =
        assertThrows(
            IllegalArgumentException.class,
            () -> saturated.ingestAt(text, Long.MAX_VALUE));
    assertTrue(atMaximum.getMessage().contains("quarantined"));
    saturated.expire(Long.MAX_VALUE);
    final IllegalArgumentException afterExpire =
        assertThrows(
            IllegalArgumentException.class,
            () -> saturated.ingestAt(text, Long.MAX_VALUE));
    assertTrue(afterExpire.getMessage().contains("quarantined"));
  }

  @Test
  public void expectedProfileAndKindMismatchesQuarantineTheirStreamIds() {
    final IrohaPeerWireMessageV1 kagemusha = kagemushaMessage(new byte[] {1});
    final String profileText = IrohaPeerQRCodecV1.encode(kagemusha).get(0);
    final IrohaPeerQRScanSessionV1 profileSession =
        new IrohaPeerQRScanSessionV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE, null, null);
    final IllegalArgumentException profileMismatch =
        assertThrows(
            IllegalArgumentException.class,
            () -> profileSession.ingestAt(profileText, 100));
    assertTrue(profileMismatch.getMessage().contains("profile mismatch"));
    final IllegalArgumentException repeatedProfile =
        assertThrows(
            IllegalArgumentException.class,
            () -> profileSession.ingestAt(profileText, 101));
    assertTrue(repeatedProfile.getMessage().contains("quarantined"));

    final IrohaPeerWireMessageV1 acknowledgement =
        new IrohaPeerWireMessageV1(
            new IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
                1,
                new byte[] {2}));
    final String kindText = IrohaPeerQRCodecV1.encode(acknowledgement).get(0);
    final IrohaPeerQRScanSessionV1 kindSession =
        new IrohaPeerQRScanSessionV1(null, IrohaPeerPayloadKind.PAYMENT, null);
    final IllegalArgumentException kindMismatch =
        assertThrows(
            IllegalArgumentException.class,
            () -> kindSession.ingestAt(kindText, 200));
    assertTrue(kindMismatch.getMessage().contains("kind mismatch"));
    final IllegalArgumentException repeatedKind =
        assertThrows(
            IllegalArgumentException.class,
            () -> kindSession.ingestAt(kindText, 201));
    assertTrue(repeatedKind.getMessage().contains("quarantined"));
  }

  @Test
  public void expiresIdleAndAbsoluteAgeDespiteDuplicateNoise() {
    final List<String> frames = IrohaPeerQRCodecV1.animatedFrameTexts(animatedMessage(9));
    final IrohaPeerQRScanLimitsV1 limits = new IrohaPeerQRScanLimitsV1(3, 12, 3_072, 10, 20);
    final IrohaPeerQRScanSessionV1 idle = new IrohaPeerQRScanSessionV1(limits);
    idle.ingestAt(frames.get(0), 100);
    idle.ingestAt(frames.get(0), 105);
    assertTrue(idle.expire(109).isEmpty());
    assertEquals(1, idle.expire(110).size());
    assertEquals(0, idle.activeStreamCount());

    final IrohaPeerQRScanSessionV1 absolute = new IrohaPeerQRScanSessionV1(limits);
    absolute.ingestAt(frames.get(0), 200);
    absolute.ingestAt(frames.get(1), 207);
    absolute.ingestAt(frames.get(2), 214);
    assertEquals(1, absolute.expire(220).size());
  }

  @Test
  public void rejectsExplicitClockRollbackUntilReset() {
    final IrohaPeerWireMessageV1 message = messageWithBytes(new byte[] {4, 2});
    final String text = IrohaPeerQRCodecV1.encode(message).get(0);
    final IrohaPeerQRScanSessionV1 session = new IrohaPeerQRScanSessionV1();
    assertTrue(session.expire(100).isEmpty());

    assertThrows(IllegalArgumentException.class, () -> session.ingestAt(text, 99));
    assertThrows(IllegalArgumentException.class, () -> session.quarantine(message.streamId(), 99));
    assertThrows(IllegalArgumentException.class, () -> session.expire(99));

    session.reset();
    assertEquals(message, session.ingestAt(text, 99).message());
  }

  @Test
  public void boundsPreheaderFramesAndBytesThenQuarantinesConflict() {
    final List<String> all = IrohaPeerQRCodecV1.animatedFrameTexts(animatedMessage(11));
    final List<String> data =
        all.stream()
            .filter(text -> IrohaPeerQRCodecV1.decodeFrame(text).frameKind()
                == IrohaPeerQRFrameV1.FrameKind.DATA)
            .toList();
    final IrohaPeerQRScanSessionV1 session =
        new IrohaPeerQRScanSessionV1(new IrohaPeerQRScanLimitsV1(3, 1, 256, 30_000, 180_000));
    session.ingestAt(data.get(0), 1);
    assertThrows(IllegalArgumentException.class, () -> session.ingestAt(data.get(1), 2));
    assertEquals(0, session.activeStreamCount());
    assertThrows(IllegalArgumentException.class, () -> session.ingestAt(data.get(0), 3));
  }

  @Test
  public void checksumCorrectHostileHeaderIsQuarantinedUntilExactExpiry() {
    final IrohaPeerWireMessageV1 message = animatedMessage(61);
    final List<IrohaPeerQRFrameV1> frames = new ArrayList<>();
    for (final String text : IrohaPeerQRCodecV1.animatedFrameTexts(message)) {
      frames.add(IrohaPeerQRCodecV1.decodeFrame(text));
    }
    final IrohaPeerQRFrameV1 validHeader = find(frames, IrohaPeerQRFrameV1.FrameKind.HEADER, 0);
    final byte[] hostilePayload = validHeader.payload();
    hostilePayload[9] = 1;
    final IrohaPeerQRFrameV1 hostileHeader =
        new IrohaPeerQRFrameV1(
            IrohaPeerQRFrameV1.FrameKind.HEADER,
            validHeader.profile(),
            validHeader.payloadKind(),
            validHeader.streamId(),
            validHeader.index(),
            validHeader.total(),
            hostilePayload);
    assertEquals(
        hostileHeader,
        IrohaPeerQRCodecV1.decodeFrame(IrohaPeerQRCodecV1.encodeFrame(hostileHeader)));

    final IrohaPeerQRScanSessionV1 session =
        new IrohaPeerQRScanSessionV1(new IrohaPeerQRScanLimitsV1(3, 12, 3_072, 10, 30));
    assertThrows(
        IllegalArgumentException.class,
        () -> session.ingestAt(IrohaPeerQRCodecV1.encodeFrame(hostileHeader), 1_000));
    assertEquals(0, session.activeStreamCount());
    assertThrows(
        IllegalArgumentException.class,
        () -> session.ingestAt(IrohaPeerQRCodecV1.encodeFrame(validHeader), 1_029));
    session.ingestAt(IrohaPeerQRCodecV1.encodeFrame(validHeader), 1_030);
    assertEquals(1, session.activeStreamCount());
  }

  @Test
  public void checksumCorrectHostileCompleteBodyAndPaddingFailClosed() {
    final IrohaPeerWireMessageV1 small = messageWithBytes(new byte[] {1, 2, 3, 4});
    final byte[] corruptMessage = small.encode();
    corruptMessage[corruptMessage.length - 1] ^= 1;
    final IrohaPeerQRFrameV1 hostileComplete =
        new IrohaPeerQRFrameV1(
            IrohaPeerQRFrameV1.FrameKind.COMPLETE,
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerPayloadKind.PAYMENT,
            small.streamId(),
            0,
            1,
            corruptMessage);
    final IrohaPeerQRScanSessionV1 completeSession = new IrohaPeerQRScanSessionV1();
    assertThrows(
        IllegalArgumentException.class,
        () -> completeSession.ingest(IrohaPeerQRCodecV1.encodeFrame(hostileComplete)));
    assertEquals(0, completeSession.activeStreamCount());

    final byte[] payload = new byte[300];
    for (int index = 0; index < payload.length; index++) payload[index] = (byte) (index * 73 + 63);
    final IrohaPeerWireMessageV1 animated = messageWithBytes(payload);
    final List<IrohaPeerQRFrameV1> frames = new ArrayList<>();
    for (final String text : IrohaPeerQRCodecV1.animatedFrameTexts(animated)) {
      frames.add(IrohaPeerQRCodecV1.decodeFrame(text));
    }
    final IrohaPeerQRFrameV1 header = find(frames, IrohaPeerQRFrameV1.FrameKind.HEADER, 0);
    final IrohaPeerQRFrameV1 first = find(frames, IrohaPeerQRFrameV1.FrameKind.DATA, 0);
    final IrohaPeerQRFrameV1 last = find(frames, IrohaPeerQRFrameV1.FrameKind.DATA, 1);
    final byte[] nonzeroPadding = last.payload();
    nonzeroPadding[nonzeroPadding.length - 1] = 1;
    final IrohaPeerQRFrameV1 hostileLast =
        new IrohaPeerQRFrameV1(
            IrohaPeerQRFrameV1.FrameKind.DATA,
            last.profile(),
            last.payloadKind(),
            last.streamId(),
            last.index(),
            last.total(),
            nonzeroPadding);
    final IrohaPeerQRScanSessionV1 animatedSession = new IrohaPeerQRScanSessionV1();
    animatedSession.ingest(IrohaPeerQRCodecV1.encodeFrame(header));
    animatedSession.ingest(IrohaPeerQRCodecV1.encodeFrame(first));
    assertThrows(
        IllegalArgumentException.class,
        () -> animatedSession.ingest(IrohaPeerQRCodecV1.encodeFrame(hostileLast)));
    assertEquals(0, animatedSession.activeStreamCount());
    assertThrows(
        IllegalArgumentException.class,
        () -> animatedSession.ingest(IrohaPeerQRCodecV1.encodeFrame(header)));
  }

  @Test
  public void expectedSchemaQuarantinesCompleteAndAnimatedRecordsUntilExpiry() {
    final IrohaPeerQRScanLimitsV1 limits =
        new IrohaPeerQRScanLimitsV1(3, 12, 3_072, 10, 30);
    final IrohaPeerWireMessageV1 small = messageWithBytes(new byte[] {7, 1});
    final String staticText = IrohaPeerQRCodecV1.encode(small).get(0);
    final IrohaPeerQRScanSessionV1 staticSession =
        new IrohaPeerQRScanSessionV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerPayloadKind.PAYMENT,
            0x0102,
            limits,
            () -> 0L);
    final IllegalArgumentException staticMismatch =
        assertThrows(
            IllegalArgumentException.class,
            () -> staticSession.ingestAt(staticText, 1_000));
    assertTrue(staticMismatch.getMessage().contains("expected 258, received 1"));
    final IllegalArgumentException staticQuarantine =
        assertThrows(
            IllegalArgumentException.class,
            () -> staticSession.ingestAt(staticText, 1_029));
    assertTrue(staticQuarantine.getMessage().contains("quarantined"));
    final IllegalArgumentException expiredStatic =
        assertThrows(
            IllegalArgumentException.class,
            () -> staticSession.ingestAt(staticText, 1_030));
    assertTrue(expiredStatic.getMessage().contains("expected 258, received 1"));

    final List<String> animated = IrohaPeerQRCodecV1.animatedFrameTexts(animatedMessage(72));
    final List<String> headers = new ArrayList<>();
    String parity = null;
    for (final String text : animated) {
      final IrohaPeerQRFrameV1 frame = IrohaPeerQRCodecV1.decodeFrame(text);
      if (frame.frameKind() == IrohaPeerQRFrameV1.FrameKind.HEADER) headers.add(text);
      if (parity == null && frame.frameKind() == IrohaPeerQRFrameV1.FrameKind.PARITY) parity = text;
    }
    assertTrue(headers.size() >= 2);
    final String trailingParityText = parity;
    final IrohaPeerQRScanSessionV1 animatedSession =
        new IrohaPeerQRScanSessionV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerPayloadKind.PAYMENT,
            0x0102,
            limits,
            () -> 0L);
    assertThrows(
        IllegalArgumentException.class,
        () -> animatedSession.ingestAt(headers.get(0), 2_000));
    final IllegalArgumentException trailingParity =
        assertThrows(
            IllegalArgumentException.class,
            () -> animatedSession.ingestAt(trailingParityText, 2_001));
    assertTrue(trailingParity.getMessage().contains("quarantined"));
    final IllegalArgumentException repeatedHeader =
        assertThrows(
            IllegalArgumentException.class,
            () -> animatedSession.ingestAt(headers.get(headers.size() - 1), 2_029));
    assertTrue(repeatedHeader.getMessage().contains("quarantined"));
    final IllegalArgumentException expiredHeader =
        assertThrows(
            IllegalArgumentException.class,
            () -> animatedSession.ingestAt(headers.get(headers.size() - 1), 2_030));
    assertTrue(expiredHeader.getMessage().contains("expected 258, received 1"));

    final IrohaPeerWireMessageV1 kagemusha = kagemushaMessage(new byte[] {1, 2, 3});
    assertEquals(
        kagemusha,
        new IrohaPeerQRScanSessionV1(
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                IrohaPeerPayloadKind.PAYMENT,
                0x0102)
            .ingest(IrohaPeerQRCodecV1.encode(kagemusha).get(0))
            .message());
  }

  private static IrohaPeerWireMessageV1 animatedMessage(final int seed) {
    int state = seed;
    final byte[] bytes = new byte[1_800];
    for (int index = 0; index < bytes.length; index++) {
      state = state * 1_664_525 + 1_013_904_223;
      bytes[index] = (byte) (state >>> 24);
    }
    return messageWithBytes(bytes);
  }

  private static IrohaPeerWireMessageV1 messageWithBytes(final byte[] bytes) {
    return new IrohaPeerWireMessageV1(
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerPayloadKind.PAYMENT,
            1,
            bytes));
  }

  private static IrohaPeerWireMessageV1 kagemushaMessage(final byte[] body) {
    final NoritoHeader header = new NoritoHeader(
        SchemaHash.hash16(
            "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4"),
        body.length,
        CRC64.compute(body),
        NoritoHeader.COMPACT_LEN,
        NoritoHeader.COMPRESSION_NONE);
    final byte[] archive = Arrays.copyOf(header.encode(), 48 + body.length);
    System.arraycopy(body, 0, archive, 48, body.length);
    return new IrohaPeerWireMessageV1(
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerPayloadKind.PAYMENT,
            0x0102,
            archive));
  }

  private static IrohaPeerQRFrameV1 find(
      final List<IrohaPeerQRFrameV1> frames,
      final IrohaPeerQRFrameV1.FrameKind kind,
      final int index) {
    for (final IrohaPeerQRFrameV1 frame : frames) {
      if (frame.frameKind() == kind && frame.index() == index) return frame;
    }
    throw new AssertionError("Missing " + kind + " frame " + index);
  }
}
