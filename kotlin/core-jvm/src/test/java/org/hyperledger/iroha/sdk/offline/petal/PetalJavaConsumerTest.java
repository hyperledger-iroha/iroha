package org.hyperledger.iroha.sdk.offline.petal;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.List;
import org.junit.jupiter.api.Test;

/** Java consumers stream, render, decode and reassemble a Petal payload through the Kotlin API. */
final class PetalJavaConsumerTest {
  private static byte[] payload(int length) {
    final PetalXorshift32 rng = new PetalXorshift32(2);
    final byte[] bytes = new byte[length];
    for (int index = 0; index < length; index++) {
      bytes[index] = (byte) rng.nextByte();
    }
    return bytes;
  }

  @Test
  void javaEncodesRendersDecodesAndReassemblesAStream() {
    final byte[] payload = payload(300);
    final PetalStreamEncoder encoder = new PetalStreamEncoder(payload, 2);
    assertEquals(PetalCrc32c.compute(payload), encoder.getMeta().getCrc());
    assertEquals(19, encoder.getMeta().getSourceAtoms());
    final PetalStreamAssembler assembler = new PetalStreamAssembler(PetalAssemblerLimits.DEFAULT);
    final PetalRenderOptions render = new PetalRenderOptions(512, 2);
    for (int frame = 0; frame < encoder.getSystematicFrames(); frame++) {
      final PetalLuma luma = PetalRenderer.render(encoder.cells(frame), render).toLuma();
      final PetalDecodeResult result = PetalDecoder.decode(luma, PetalDecodeOptions.DEFAULT);
      assertTrue(result.isSuccess(), "frame " + frame);
      assertNull(result.getError());
      final PetalDecodedFrame decoded = result.getFrame();
      assertEquals("PKD", decoded.getLanes());
      assertArrayEquals(encoder.laneData(frame).p(), decoded.getP().getData());
      if (PetalStream.isBeaconFrame(frame)) {
        assertEquals(encoder.getMeta(), decoded.beacon().getMeta());
      }
      decoded.feed(assembler);
    }
    final PetalProgress progress = assembler.progress();
    assertTrue(progress.getComplete());
    assertEquals(19, progress.getRank());
    final PetalCompleted done = assembler.takeCompleted();
    assertNotNull(done);
    assertArrayEquals(payload, done.getPayload());
    assertEquals(2, done.getMeta().getKind());
    assertNull(assembler.takeCompleted());
  }

  @Test
  void javaScansCameraPlanesThroughASession() {
    final byte[] payload = "Petal Stream from Java".getBytes(StandardCharsets.UTF_8);
    final PetalStreamEncoder encoder = new PetalStreamEncoder(payload, 1);
    final PetalScanSession session = new PetalScanSession(new PetalScanLimits(5_000L, 60_000L));
    final PetalLuma frame = PetalRenderer.render(encoder.cells(0), new PetalRenderOptions(480, 2)).toLuma();
    // a camera-style plane: 8 bytes of row padding, pixel stride 1
    final int rowStride = frame.getWidth() + 8;
    final ByteBuffer plane = ByteBuffer.allocateDirect(rowStride * frame.getHeight());
    final byte[] pixels = frame.data();
    for (int row = 0; row < frame.getHeight(); row++) {
      plane.position(row * rowStride);
      plane.put(pixels, row * frame.getWidth(), frame.getWidth());
    }
    plane.rewind();
    final PetalLuma camera = PetalLuma.fromPlane(plane, frame.getWidth(), frame.getHeight(), rowStride);
    assertEquals(frame, camera);
    final PetalScanOutcome outcome = session.push(camera, 0L);
    assertNull(outcome.getError());
    assertNotNull(outcome.getCompleted());
    assertArrayEquals(payload, outcome.getCompleted().getPayload());
    assertEquals(1L, session.stats().getFrames());
    final PetalScanOutcome blank = session.push(new PetalLuma(320, 240), 10L);
    assertEquals(PetalDecodeError.NO_FINDERS, blank.getError());
    assertEquals("", blank.getLanes());
  }

  @Test
  void javaReadsTheCodecAndDrawListSurface() {
    final byte[] data = new byte[PetalLanes.P_DATA];
    final byte[] word = PetalLanes.encodeLane(PetalLane.P, data);
    word[0] ^= 0x5A;
    assertArrayEquals(data, PetalLanes.decodeLane(PetalLane.P, word));
    final PetalReedSolomonException failure =
        assertThrows(PetalReedSolomonException.class, () -> PetalLanes.decodeLane(PetalLane.D, new byte[3]));
    assertEquals(PetalReedSolomonException.Reason.INVALID_SHAPE, failure.getReason());

    final PetalStreamEncoder encoder = new PetalStreamEncoder(payload(64), 3);
    final PetalDrawList list = PetalDrawList.of(encoder.cells(5), PetalPalette.DEFAULT);
    assertEquals(PetalLayout.TILE_COUNT, list.getTiles().size());
    assertEquals(24, list.getFinderDiscs().size());
    assertFalse(list.getDots().isEmpty());
    final List<double[]> strokes = PetalGlyphs.strokes(list.getTiles().get(0).getGlyph());
    assertFalse(strokes.isEmpty());
    assertEquals(PetalLayout.GLYPH_BOX / PetalGlyphs.GLYPH_GRID, PetalDrawList.GLYPH_SCALE);
    final PetalDLane lane = PetalStream.parseDLane(encoder.laneData(4).d());
    assertNotNull(lane);
    assertTrue(lane.isBeacon());
    assertEquals(PetalStream.firstAtomId(5), PetalStream.parseAtomLane(PetalLane.P, encoder.laneData(5).p()).getFirstId());
  }
}
