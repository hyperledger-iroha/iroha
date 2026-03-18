package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.List;

public final class OfflinePetalStreamTest {
  private OfflinePetalStreamTest() {}

  public static void main(final String[] args) {
    roundTripPayload();
    sampleRoundTrip();
    encodeGridsUsesMaxPayload();
    System.out.println("[IrohaAndroid] OfflinePetalStreamTest passed.");
  }

  private static void roundTripPayload() {
    final byte[] payload = makePayload(128);
    final OfflinePetalStream.Grid grid =
        OfflinePetalStream.Encoder.encodeGrid(payload, new OfflinePetalStream.Options());
    final byte[] decoded = OfflinePetalStream.Decoder.decodeGrid(grid, new OfflinePetalStream.Options());
    assertArrayEquals(payload, decoded, "petal grid payload mismatch");
  }

  private static void sampleRoundTrip() {
    final byte[] payload = makePayload(200);
    final OfflinePetalStream.Grid grid =
        OfflinePetalStream.Encoder.encodeGrid(payload, new OfflinePetalStream.Options());
    final byte[] rgba = renderGridToRgba(grid, 4);
    final OfflinePetalStream.SampleGrid samples =
        OfflinePetalStream.Sampler.sampleGridFromRgba(
            rgba, grid.gridSize() * 4, grid.gridSize() * 4, grid.gridSize());
    final byte[] decoded =
        OfflinePetalStream.Decoder.decodeSamples(samples, new OfflinePetalStream.Options());
    assertArrayEquals(payload, decoded, "petal sample payload mismatch");
  }

  private static void encodeGridsUsesMaxPayload() {
    final byte[] small = makePayload(16);
    final byte[] large = makePayload(240);
    final OfflinePetalStream.EncodeResult result =
        OfflinePetalStream.Encoder.encodeGrids(
            Arrays.asList(small, large), new OfflinePetalStream.Options());
    assertTrue(result.grids().size() == 2, "grid count mismatch");
    assertTrue(result.gridSize() == result.grids().get(0).gridSize(), "grid size mismatch");
    assertTrue(result.gridSize() == result.grids().get(1).gridSize(), "grid size mismatch");
  }

  private static byte[] renderGridToRgba(final OfflinePetalStream.Grid grid, final int cellSize) {
    final int size = grid.gridSize() * cellSize;
    final byte[] data = new byte[size * size * 4];
    for (int y = 0; y < grid.gridSize(); y++) {
      for (int x = 0; x < grid.gridSize(); x++) {
        final int idx = y * grid.gridSize() + x;
        final byte value = grid.cells()[idx] ? (byte) 0x00 : (byte) 0xFF;
        for (int oy = 0; oy < cellSize; oy++) {
          for (int ox = 0; ox < cellSize; ox++) {
            final int px = x * cellSize + ox;
            final int py = y * cellSize + oy;
            final int offset = (py * size + px) * 4;
            data[offset] = value;
            data[offset + 1] = value;
            data[offset + 2] = value;
            data[offset + 3] = (byte) 0xFF;
          }
        }
      }
    }
    return data;
  }

  private static byte[] makePayload(final int length) {
    final byte[] payload = new byte[length];
    for (int i = 0; i < payload.length; i++) {
      payload[i] = (byte) ((i * 17 + 13) & 0xFF);
    }
    return payload;
  }

  private static void assertTrue(final boolean condition, final String message) {
    if (!condition) {
      throw new AssertionError(message);
    }
  }

  private static void assertArrayEquals(
      final byte[] expected, final byte[] actual, final String message) {
    if (!Arrays.equals(expected, actual)) {
      throw new AssertionError(message);
    }
  }
}
