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
      assertEquals("D0123", decoded.getLanes());
      assertArrayEquals(encoder.laneData(frame).field(0), decoded.field(0).getData());
      assertArrayEquals(encoder.laneData(frame).d(), decoded.getD().getData());
      if (PetalStream.isBeaconFrame(frame)) {
        assertEquals(encoder.getMeta(), decoded.beacon().getMeta());
      }
      // the katakana bands of the card carry four more lanes
      final List<PetalKanaResult> kana =
          PetalKana.readKana(luma, decoded, decoded.atomPackets().get(0).getHeader());
      for (final PetalKanaLane lane : PetalKanaLane.ALL) {
        assertArrayEquals(encoder.laneData(frame).kana(lane), kana.get(lane.ordinal()).getResult().getData());
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
  void javaFollowsCandidatesAndTracksAPose() {
    final PetalStreamEncoder encoder = new PetalStreamEncoder(payload(300), 2);
    final PetalLuma luma = PetalRenderer.render(encoder.cells(3), new PetalRenderOptions(512, 2)).toLuma();
    final java.util.Iterator<PetalFinderSet> candidates = PetalLocator.candidates(luma);
    assertTrue(candidates.hasNext());
    final PetalFinderSet first = candidates.next();
    assertNull(first.getInferred());
    assertEquals(PetalLocator.locate(luma), first.getCorners());
    final PetalDecodedFrame decoded = PetalDecoder.decode(luma).getFrame();
    assertNotNull(decoded);
    assertNull(decoded.getInferredCorner());
    final PetalDecodedFrame followed = PetalDecoder.track(luma, decoded);
    assertNotNull(followed);
    assertEquals("D0123", followed.getLanes());
    assertEquals(0.0, followed.getPose().getDistortion());
    assertNotNull(PetalDecoder.decodeAt(luma, followed.getPose()));
    // without lens distortion a pose is its homography
    final PetalPose plain = PetalPose.plain(followed.getPose().getHomography());
    assertArrayEquals(followed.getPose().apply(100.0, 200.0), plain.apply(100.0, 200.0));
    assertEquals(500L, PetalScanSession.TRACK_WINDOW_MILLIS);
    final PetalScanSession session = new PetalScanSession();
    session.push(luma, 0L);
    session.push(luma, PetalScanSession.TRACK_WINDOW_MILLIS);
    assertEquals(1L, session.stats().getTracked());
    assertEquals(0L, session.stats().getInferred());
  }

  @Test
  void javaReadsTheTurnLanesOfACapture() {
    final PetalStreamEncoder encoder = new PetalStreamEncoder(payload(3000), 2);
    final PetalLuma luma = PetalRenderer.render(encoder.cells(5), new PetalRenderOptions(512, 2)).toLuma();
    final PetalDecodedFrame decoded = PetalDecoder.decode(luma).getFrame();
    assertNotNull(decoded);
    final List<PetalLaneResult> turns = PetalTurns.readTurns(luma, decoded);
    assertEquals(4, turns.size());
    for (int sector = 0; sector < 4; sector++) {
      assertArrayEquals(encoder.laneData(5).turn(sector), turns.get(sector).getData());
      final byte[] field = decoded.field(sector).getData();
      final PetalLaneHeader header = new PetalLaneHeader(field[0] & 0xFF, ((field[1] & 0xFF) << 8) | (field[2] & 0xFF));
      final PetalAtomPacket packet = PetalTurns.turnPacket(header, sector, turns.get(sector).getData());
      assertNotNull(packet);
      assertEquals(PetalTurns.turnFirstId(5, sector), packet.getFirstId());
    }
    final List<PetalLaneResult> skipped = PetalTurns.readTurns(luma, decoded, new boolean[] {true, true, false, true});
    assertNull(skipped.get(0));
    assertNotNull(skipped.get(2));
    final PetalObservedTurns observed = PetalTurns.observedTurns(luma, decoded);
    assertNotNull(observed);
    assertEquals(4 * PetalTurns.TURN_CARRIERS, observed.getCarriers().size());
    assertEquals(Integer.MIN_VALUE, PetalTurns.TURN_ID_BASE);
    assertEquals(90, PetalTurns.TURN_WORD);
    // a session reads a shown frame's bands on its first capture, its turn lanes on the next, each once
    final PetalScanSession session = new PetalScanSession();
    final PetalScanOutcome first = session.push(luma, 0L);
    assertEquals("P0K0P1K1", first.getKana());
    assertEquals("", first.getTurned());
    assertEquals("0123", session.push(luma, 33L).getTurned());
    assertEquals("", session.push(luma, 66L).getTurned());
    assertEquals(4L, session.stats().getTurnLanes());
    assertEquals(4L, session.stats().getKanaLanes());
  }

  @Test
  void javaReadsTheCodecAndDrawListSurface() {
    final byte[] data = new byte[PetalLanes.FIELD_DATA];
    final PetalLane lane = PetalLane.field(2);
    assertEquals(PetalLane.F2, lane);
    assertEquals('2', lane.getLetter());
    final byte[] word = PetalLanes.encodeLane(lane, data);
    word[0] ^= 0x5A;
    assertArrayEquals(data, PetalLanes.decodeLane(lane, word));
    assertEquals(5, PetalLane.ALL.size());
    final PetalReedSolomonException failure =
        assertThrows(PetalReedSolomonException.class, () -> PetalLanes.decodeLane(PetalLane.D, new byte[3]));
    assertEquals(PetalReedSolomonException.Reason.INVALID_SHAPE, failure.getReason());

    final PetalStreamEncoder encoder = new PetalStreamEncoder(payload(64), 3);
    final PetalDrawList list = PetalDrawList.of(encoder.cells(5), PetalPalette.DEFAULT);
    assertEquals(24, list.getFinderDiscs().size());
    assertFalse(list.getDots().isEmpty());
    final List<PetalDrawPetal> petals = list.getPetals();
    assertFalse(petals.isEmpty());
    assertEquals(PetalLayout.isLightCell(petals.get(0).getCell()), petals.get(0).getLight());
    assertFalse(list.getBackdropCells().isEmpty());
    assertEquals(PetalLayout.BACKDROP_RADIUS, PetalDrawList.BACKDROP_RADIUS);
    assertEquals(6.0, PetalDrawList.PETAL_HALF_LENGTH);
    assertEquals(3.73, PetalDrawList.PETAL_HALF_WIDTH);
    final PetalDrawPetal first = petals.get(0);
    assertEquals(
        PetalLayout.cellAngle(first.getCell()) + PetalTurns.turnAngle(first.getTurn()), first.getAngle());
    final PetalDLane dLane = PetalStream.parseDLane(encoder.laneData(4).d());
    assertNotNull(dLane);
    assertTrue(dLane.isBeacon());
    assertEquals(
        PetalStream.firstAtomId(5) + 5,
        PetalStream.parseAtomLane(PetalLane.F1, encoder.laneData(5).field(1)).getFirstId());
  }
}
