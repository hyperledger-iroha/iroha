package org.hyperledger.iroha.android.offline;

import java.util.List;

public final class OfflineQrStreamTest {

  private OfflineQrStreamTest() {}

  public static void main(final String[] args) {
    roundTripPayload();
    recoversMissingChunk();
    rejectsBadChecksum();
    textCodecRoundTrip();
    sakuraStormPlaybackSkinMatchesPreset();
    sakuraStormScanSessionPresetRecoversDroppedFrame();
    System.out.println("[IrohaAndroid] OfflineQrStreamTest passed.");
  }

  private static void roundTripPayload() {
    final byte[] payload = makePayload(1024);
    final List<byte[]> frames =
        OfflineQrStream.Encoder.encodeFrameBytes(
            payload, OfflineQrStream.PayloadKind.UNSPECIFIED, new OfflineQrStream.Options(200, 0));
    final OfflineQrStream.Decoder decoder = new OfflineQrStream.Decoder();
    OfflineQrStream.DecodeResult result = null;
    for (byte[] frame : frames) {
      result = decoder.ingest(frame);
    }
    assertNotNull(result, "decode result");
    assertTrue(result.isComplete(), "roundtrip incomplete");
    assertArrayEquals(payload, result.payload(), "roundtrip payload mismatch");
  }

  private static void recoversMissingChunk() {
    final byte[] payload = makePayload(900);
    final List<OfflineQrStream.Frame> frames =
        OfflineQrStream.Encoder.encodeFrames(
            payload,
            OfflineQrStream.PayloadKind.OFFLINE_SPEND_RECEIPT,
            new OfflineQrStream.Options(180, 3));
    OfflineQrStream.Frame header = null;
    int droppedIndex = -1;
    for (OfflineQrStream.Frame frame : frames) {
      if (frame.kind() == OfflineQrStream.FrameKind.HEADER) {
        header = frame;
        break;
      }
    }
    assertNotNull(header, "header");
    final OfflineQrStream.Decoder decoder = new OfflineQrStream.Decoder();
    decoder.ingest(header.encode());
    OfflineQrStream.DecodeResult result = null;
    for (OfflineQrStream.Frame frame : frames) {
      if (frame.kind() == OfflineQrStream.FrameKind.DATA && frame.index() == 1) {
        droppedIndex = frame.index();
        continue;
      }
      if (frame.kind() != OfflineQrStream.FrameKind.HEADER) {
        result = decoder.ingest(frame.encode());
      }
    }
    assertTrue(droppedIndex == 1, "dropped index mismatch");
    assertNotNull(result, "decode result");
    assertTrue(result.isComplete(), "parity recovery incomplete");
    assertArrayEquals(payload, result.payload(), "parity payload mismatch");
  }

  private static void rejectsBadChecksum() {
    final byte[] payload = makePayload(400);
    final List<byte[]> frames =
        OfflineQrStream.Encoder.encodeFrameBytes(
            payload, OfflineQrStream.PayloadKind.UNSPECIFIED, new OfflineQrStream.Options());
    final byte[] corrupted = frames.get(0).clone();
    corrupted[corrupted.length - 1] ^= (byte) 0x11;
    final OfflineQrStream.Decoder decoder = new OfflineQrStream.Decoder();
    boolean threw = false;
    try {
      decoder.ingest(corrupted);
    } catch (IllegalArgumentException error) {
      threw = true;
    }
    assertTrue(threw, "checksum mismatch should throw");
  }

  private static void textCodecRoundTrip() {
    final byte[] payload = makePayload(128);
    final String encoded =
        OfflineQrStream.TextCodec.encode(payload, OfflineQrStream.FrameEncoding.BASE64);
    final byte[] decoded =
        OfflineQrStream.TextCodec.decode(encoded, OfflineQrStream.FrameEncoding.BASE64);
    assertArrayEquals(payload, decoded, "text codec payload mismatch");
  }

  private static void sakuraStormPlaybackSkinMatchesPreset() {
    final OfflineQrStream.PlaybackSkin skin = OfflineQrStream.SAKURA_STORM_SKIN;
    assertTrue("sakura-storm".equals(skin.name), "storm skin name mismatch");
    assertTrue(skin.frameRate == 12.0, "storm frameRate mismatch");
    assertTrue(skin.petalDriftSpeed == 0.6, "storm drift mismatch");
    assertTrue(skin.progressOverlayAlpha == 0.34, "storm overlay mismatch");
    assertTrue(skin.theme.backgroundStart.red == 0.05, "storm backgroundStart mismatch");
    assertTrue(skin.theme.backgroundEnd.blue == 0.04, "storm backgroundEnd mismatch");
  }

  private static void sakuraStormScanSessionPresetRecoversDroppedFrame() {
    final byte[] payload = makePayload(6 * 1024);
    final List<OfflineQrStream.Frame> frames =
        OfflineQrStream.Encoder.encodeFrames(
            payload,
            OfflineQrStream.PayloadKind.OFFLINE_SPEND_RECEIPT,
            new OfflineQrStream.Options(336, 4));

    OfflineQrStream.Frame header = null;
    for (OfflineQrStream.Frame frame : frames) {
      if (frame.kind() == OfflineQrStream.FrameKind.HEADER) {
        header = frame;
        break;
      }
    }
    assertNotNull(header, "header");

    final OfflineQrStream.ScanSession session = new OfflineQrStream.ScanSession();
    session.ingest(header.encode());

    OfflineQrStream.DecodeResult result = null;
    for (OfflineQrStream.Frame frame : frames) {
      if (frame.kind() == OfflineQrStream.FrameKind.DATA && frame.index() == 1) {
        continue;
      }
      if (frame.kind() == OfflineQrStream.FrameKind.HEADER) {
        continue;
      }
      result = session.ingest(frame.encode());
    }

    assertNotNull(result, "storm decode result");
    assertTrue(result.isComplete(), "storm scan session incomplete");
    assertArrayEquals(payload, result.payload(), "storm scan payload mismatch");
    assertTrue(result.recoveredChunks() == 1, "storm recovered chunk mismatch");
  }

  private static byte[] makePayload(final int length) {
    final byte[] payload = new byte[length];
    for (int i = 0; i < payload.length; i++) {
      payload[i] = (byte) ((i * 31 + 7) & 0xFF);
    }
    return payload;
  }

  private static void assertNotNull(final Object value, final String label) {
    if (value == null) {
      throw new AssertionError(label + " must not be null");
    }
  }

  private static void assertTrue(final boolean condition, final String message) {
    if (!condition) {
      throw new AssertionError(message);
    }
  }

  private static void assertArrayEquals(
      final byte[] expected, final byte[] actual, final String message) {
    if (expected == null && actual == null) {
      return;
    }
    if (expected == null || actual == null || expected.length != actual.length) {
      throw new AssertionError(message);
    }
    for (int i = 0; i < expected.length; i++) {
      if (expected[i] != actual[i]) {
        throw new AssertionError(message);
      }
    }
  }
}
