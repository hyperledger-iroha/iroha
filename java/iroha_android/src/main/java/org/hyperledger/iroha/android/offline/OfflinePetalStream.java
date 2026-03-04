package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;
import java.util.zip.CRC32;

/** Petal stream framing helpers for offline payload transfer. */
public final class OfflinePetalStream {

  private static final byte[] MAGIC = {(byte) 0x50, (byte) 0x53};
  private static final byte VERSION = 1;
  private static final int HEADER_LEN = 9;
  private static final int[] GRID_SIZES = {33, 37, 41, 45, 49, 53, 57, 61, 65, 69};

  private OfflinePetalStream() {}

  public static int[] gridSizes() {
    return GRID_SIZES.clone();
  }

  public static final class Options {
    private final int gridSize;
    private final int border;
    private final int anchorSize;

    public Options() {
      this(0, 1, 3);
    }

    public Options(final int gridSize, final int border, final int anchorSize) {
      if (gridSize < 0 || gridSize > 0xFFFF) {
        throw new IllegalArgumentException("gridSize must be between 0 and 65535");
      }
      if (border <= 0 || border > 0xFF) {
        throw new IllegalArgumentException("border must be between 1 and 255");
      }
      if (anchorSize <= 0 || anchorSize > 0xFF) {
        throw new IllegalArgumentException("anchorSize must be between 1 and 255");
      }
      this.gridSize = gridSize;
      this.border = border;
      this.anchorSize = anchorSize;
    }

    public int gridSize() {
      return gridSize;
    }

    public int border() {
      return border;
    }

    public int anchorSize() {
      return anchorSize;
    }
  }

  public static final class Grid {
    private final int gridSize;
    private final boolean[] cells;

    public Grid(final int gridSize, final boolean[] cells) {
      if (gridSize <= 0) {
        throw new IllegalArgumentException("gridSize must be > 0");
      }
      Objects.requireNonNull(cells, "cells");
      if (cells.length != gridSize * gridSize) {
        throw new IllegalArgumentException("cells length does not match grid size");
      }
      this.gridSize = gridSize;
      this.cells = cells.clone();
    }

    public int gridSize() {
      return gridSize;
    }

    public boolean get(final int x, final int y) {
      if (x < 0 || y < 0 || x >= gridSize || y >= gridSize) {
        return false;
      }
      return cells[y * gridSize + x];
    }

    public boolean[] cells() {
      return cells.clone();
    }
  }

  public static final class SampleGrid {
    private final int gridSize;
    private final int[] samples;

    public SampleGrid(final int gridSize, final int[] samples) {
      if (gridSize <= 0) {
        throw new IllegalArgumentException("gridSize must be > 0");
      }
      Objects.requireNonNull(samples, "samples");
      if (samples.length != gridSize * gridSize) {
        throw new IllegalArgumentException("samples length does not match grid size");
      }
      this.gridSize = gridSize;
      this.samples = samples.clone();
    }

    public int gridSize() {
      return gridSize;
    }

    public int[] samples() {
      return samples.clone();
    }
  }

  public static final class Encoder {
    private Encoder() {}

    public static Grid encodeGrid(final byte[] payload, final Options options) {
      Objects.requireNonNull(payload, "payload");
      final Options normalized = options == null ? new Options() : options;
      if (payload.length > 0xFFFF) {
        throw new IllegalArgumentException("payload length exceeds 65535");
      }
      final int resolved = resolveGridSize(payload.length, normalized);
      final int capacity = capacityBits(resolved, normalized);
      final int bitsNeeded = (HEADER_LEN + payload.length) * 8;
      if (bitsNeeded > capacity) {
        throw new IllegalArgumentException("petal stream grid too small for payload");
      }
      final byte[] header = encodeHeader(payload);
      final List<Boolean> bits = new ArrayList<>(bitsNeeded);
      pushBytesAsBits(header, bits);
      pushBytesAsBits(payload, bits);
      final boolean[] cells = new boolean[resolved * resolved];
      int bitIndex = 0;
      for (int y = 0; y < resolved; y++) {
        for (int x = 0; x < resolved; x++) {
          final int idx = y * resolved + x;
          final CellRole role = cellRole(x, y, resolved, normalized);
          if (role == CellRole.BORDER || role == CellRole.ANCHOR_DARK) {
            cells[idx] = true;
          } else if (role == CellRole.ANCHOR_LIGHT) {
            cells[idx] = false;
          } else if (bitIndex < bits.size()) {
            cells[idx] = bits.get(bitIndex);
            bitIndex += 1;
          }
        }
      }
      return new Grid(resolved, cells);
    }

    public static EncodeResult encodeGrids(final List<byte[]> payloads, final Options options) {
      Objects.requireNonNull(payloads, "payloads");
      final Options normalized = options == null ? new Options() : options;
      int maxLen = 0;
      for (byte[] payload : payloads) {
        if (payload != null && payload.length > maxLen) {
          maxLen = payload.length;
        }
      }
      final int resolved = normalized.gridSize() == 0
          ? resolveGridSize(maxLen, normalized)
          : normalized.gridSize();
      final Options fixed = new Options(resolved, normalized.border(), normalized.anchorSize());
      final List<Grid> grids = new ArrayList<>(payloads.size());
      for (byte[] payload : payloads) {
        grids.add(encodeGrid(payload, fixed));
      }
      return new EncodeResult(resolved, grids);
    }
  }

  public static final class EncodeResult {
    private final int gridSize;
    private final List<Grid> grids;

    public EncodeResult(final int gridSize, final List<Grid> grids) {
      this.gridSize = gridSize;
      this.grids = grids;
    }

    public int gridSize() {
      return gridSize;
    }

    public List<Grid> grids() {
      return grids;
    }
  }

  public static final class Decoder {
    private Decoder() {}

    public static byte[] decodeGrid(final Grid grid, final Options options) {
      final Options normalized = options == null ? new Options() : options;
      final int resolved = resolveGridSizeForDecode(grid.gridSize(), normalized);
      if (resolved != grid.gridSize()) {
        throw new IllegalArgumentException("grid size mismatch");
      }
      final List<Boolean> bits = new ArrayList<>();
      for (int y = 0; y < resolved; y++) {
        for (int x = 0; x < resolved; x++) {
          if (cellRole(x, y, resolved, normalized) == CellRole.DATA) {
            bits.add(grid.get(x, y));
          }
        }
      }
      final byte[] bytes = bitsToBytes(bits);
      return decodePayload(bytes);
    }

    public static byte[] decodeSamples(final SampleGrid samples, final Options options) {
      final Options normalized = options == null ? new Options() : options;
      final int resolved = resolveGridSizeForDecode(samples.gridSize(), normalized);
      if (resolved != samples.gridSize()) {
        throw new IllegalArgumentException("sample grid size mismatch");
      }
      long darkSum = 0;
      long lightSum = 0;
      long darkCount = 0;
      long lightCount = 0;
      final int[] values = samples.samples();
      for (int y = 0; y < resolved; y++) {
        for (int x = 0; x < resolved; x++) {
          final int idx = y * resolved + x;
          final int value = values[idx];
          final CellRole role = cellRole(x, y, resolved, normalized);
          if (role == CellRole.ANCHOR_DARK) {
            darkSum += value;
            darkCount += 1;
          } else if (role == CellRole.ANCHOR_LIGHT) {
            lightSum += value;
            lightCount += 1;
          }
        }
      }
      if (darkCount == 0 || lightCount == 0) {
        throw new IllegalArgumentException("anchor sampling failed");
      }
      final double darkAvg = (double) darkSum / (double) darkCount;
      final double lightAvg = (double) lightSum / (double) lightCount;
      if (darkAvg >= lightAvg) {
        throw new IllegalArgumentException("anchor contrast too low");
      }
      final int threshold = (int) Math.round((darkAvg + lightAvg) / 2.0);
      final boolean[] cells = new boolean[values.length];
      for (int i = 0; i < values.length; i++) {
        cells[i] = values[i] < threshold;
      }
      return decodeGrid(new Grid(resolved, cells), normalized);
    }
  }

  public static final class Sampler {
    private Sampler() {}

    public static SampleGrid sampleGridFromRgba(
        final byte[] rgba, final int width, final int height, final int gridSize) {
      Objects.requireNonNull(rgba, "rgba");
      if (width <= 0 || height <= 0) {
        throw new IllegalArgumentException("image dimensions must be positive");
      }
      if (gridSize <= 0) {
        throw new IllegalArgumentException("gridSize must be > 0");
      }
      final int size = Math.min(width, height);
      final int offsetX = (width - size) / 2;
      final int offsetY = (height - size) / 2;
      final int cellSize = Math.max(1, size / gridSize);
      final int[] samples = new int[gridSize * gridSize];
      int out = 0;
      for (int y = 0; y < gridSize; y++) {
        for (int x = 0; x < gridSize; x++) {
          long sum = 0;
          long count = 0;
          for (double oy : new double[] {0.25, 0.5, 0.75}) {
            for (double ox : new double[] {0.25, 0.5, 0.75}) {
              final int px = Math.min(width - 1, offsetX + (int) Math.floor((x + ox) * cellSize));
              final int py = Math.min(height - 1, offsetY + (int) Math.floor((y + oy) * cellSize));
              final int idx = (py * width + px) * 4;
              if (idx + 2 >= rgba.length) {
                continue;
              }
              final int r = rgba[idx] & 0xFF;
              final int g = rgba[idx + 1] & 0xFF;
              final int b = rgba[idx + 2] & 0xFF;
              final int luma = (77 * r + 150 * g + 29 * b) >> 8;
              sum += luma;
              count += 1;
            }
          }
          samples[out++] = count == 0 ? 0 : (int) (sum / count);
        }
      }
      return new SampleGrid(gridSize, samples);
    }

    public static SampleGrid sampleGridFromLuma(
        final byte[] luma, final int width, final int height, final int gridSize) {
      Objects.requireNonNull(luma, "luma");
      if (width <= 0 || height <= 0) {
        throw new IllegalArgumentException("image dimensions must be positive");
      }
      if (gridSize <= 0) {
        throw new IllegalArgumentException("gridSize must be > 0");
      }
      final int size = Math.min(width, height);
      final int offsetX = (width - size) / 2;
      final int offsetY = (height - size) / 2;
      final int cellSize = Math.max(1, size / gridSize);
      final int[] samples = new int[gridSize * gridSize];
      int out = 0;
      for (int y = 0; y < gridSize; y++) {
        for (int x = 0; x < gridSize; x++) {
          long sum = 0;
          long count = 0;
          for (double oy : new double[] {0.25, 0.5, 0.75}) {
            for (double ox : new double[] {0.25, 0.5, 0.75}) {
              final int px = Math.min(width - 1, offsetX + (int) Math.floor((x + ox) * cellSize));
              final int py = Math.min(height - 1, offsetY + (int) Math.floor((y + oy) * cellSize));
              final int idx = py * width + px;
              if (idx >= luma.length) {
                continue;
              }
              sum += luma[idx] & 0xFF;
              count += 1;
            }
          }
          samples[out++] = count == 0 ? 0 : (int) (sum / count);
        }
      }
      return new SampleGrid(gridSize, samples);
    }
  }

  private enum CellRole {
    BORDER,
    ANCHOR_DARK,
    ANCHOR_LIGHT,
    DATA,
  }

  private static int resolveGridSize(final int payloadLength, final Options options) {
    if (options.border() <= 0) {
      throw new IllegalArgumentException("border must be > 0");
    }
    if (options.anchorSize() <= 0) {
      throw new IllegalArgumentException("anchorSize must be > 0");
    }
    final int bitsNeeded = (HEADER_LEN + payloadLength) * 8;
    if (options.gridSize() != 0) {
      final int capacity = capacityBits(options.gridSize(), options);
      if (bitsNeeded > capacity) {
        throw new IllegalArgumentException("petal stream grid too small for payload");
      }
      return options.gridSize();
    }
    for (int candidate : GRID_SIZES) {
      final int capacity = capacityBits(candidate, options);
      if (bitsNeeded <= capacity) {
        return candidate;
      }
    }
    throw new IllegalArgumentException("petal stream grid too small for payload");
  }

  private static int resolveGridSizeForDecode(final int gridSize, final Options options) {
    if (options.gridSize() != 0) {
      return options.gridSize();
    }
    if (gridSize == 0) {
      throw new IllegalArgumentException("grid size is zero");
    }
    return gridSize;
  }

  private static int capacityBits(final int gridSize, final Options options) {
    if (gridSize <= 0) {
      throw new IllegalArgumentException("grid size must be > 0");
    }
    final int minGrid = options.border() * 2 + options.anchorSize() * 2 + 1;
    if (gridSize < minGrid) {
      throw new IllegalArgumentException("grid size too small for anchors");
    }
    final int total = gridSize * gridSize;
    final int borderCells = gridSize * 4 - 4;
    final int anchorCells = options.anchorSize() * options.anchorSize() * 4;
    final int dataCells = Math.max(0, total - borderCells - anchorCells);
    return dataCells;
  }

  private static CellRole cellRole(
      final int x, final int y, final int gridSize, final Options options) {
    final int border = options.border();
    final int anchor = options.anchorSize();
    if (x < border || y < border || x >= gridSize - border || y >= gridSize - border) {
      return CellRole.BORDER;
    }
    final int right = gridSize - border - anchor;
    final int bottom = gridSize - border - anchor;
    final boolean inLeft = x >= border && x < border + anchor;
    final boolean inRight = x >= right && x < right + anchor;
    final boolean inTop = y >= border && y < border + anchor;
    final boolean inBottom = y >= bottom && y < bottom + anchor;
    if (inLeft && inTop) {
      return CellRole.ANCHOR_DARK;
    }
    if (inLeft && inBottom) {
      return CellRole.ANCHOR_DARK;
    }
    if (inRight && inTop) {
      return CellRole.ANCHOR_LIGHT;
    }
    if (inRight && inBottom) {
      return CellRole.ANCHOR_LIGHT;
    }
    return CellRole.DATA;
  }

  private static byte[] encodeHeader(final byte[] payload) {
    final byte[] out = new byte[HEADER_LEN];
    System.arraycopy(MAGIC, 0, out, 0, MAGIC.length);
    out[2] = VERSION;
    writeUInt16LE(out, 3, payload.length);
    final CRC32 crc32 = new CRC32();
    crc32.update(payload);
    writeUInt32LE(out, 5, crc32.getValue());
    return out;
  }

  private static byte[] decodePayload(final byte[] bytes) {
    if (bytes.length < HEADER_LEN) {
      throw new IllegalArgumentException("petal stream header too short");
    }
    if (bytes[0] != MAGIC[0] || bytes[1] != MAGIC[1]) {
      throw new IllegalArgumentException("petal stream magic mismatch");
    }
    if (bytes[2] != VERSION) {
      throw new IllegalArgumentException("petal stream version mismatch");
    }
    final int payloadLength = readUInt16LE(bytes, 3);
    final long expected = readUInt32LE(bytes, 5);
    final int start = HEADER_LEN;
    final int end = start + payloadLength;
    if (end > bytes.length) {
      throw new IllegalArgumentException("petal stream payload length exceeds data");
    }
    final byte[] payload = Arrays.copyOfRange(bytes, start, end);
    final CRC32 crc32 = new CRC32();
    crc32.update(payload);
    if (crc32.getValue() != expected) {
      throw new IllegalArgumentException("petal stream checksum mismatch");
    }
    return payload;
  }

  private static void pushBytesAsBits(final byte[] bytes, final List<Boolean> out) {
    for (byte value : bytes) {
      for (int bit = 7; bit >= 0; bit--) {
        out.add((value & (1 << bit)) != 0);
      }
    }
  }

  private static byte[] bitsToBytes(final List<Boolean> bits) {
    final byte[] out = new byte[(bits.size() + 7) / 8];
    int offset = 0;
    int index = 0;
    while (offset < bits.size()) {
      int value = 0;
      for (int bit = 0; bit < 8 && offset + bit < bits.size(); bit++) {
        if (bits.get(offset + bit)) {
          value |= 1 << (7 - bit);
        }
      }
      out[index++] = (byte) value;
      offset += 8;
    }
    return out;
  }

  private static void writeUInt16LE(final byte[] out, final int offset, final int value) {
    out[offset] = (byte) (value & 0xFF);
    out[offset + 1] = (byte) ((value >> 8) & 0xFF);
  }

  private static void writeUInt32LE(final byte[] out, final int offset, final long value) {
    out[offset] = (byte) (value & 0xFF);
    out[offset + 1] = (byte) ((value >> 8) & 0xFF);
    out[offset + 2] = (byte) ((value >> 16) & 0xFF);
    out[offset + 3] = (byte) ((value >> 24) & 0xFF);
  }

  private static int readUInt16LE(final byte[] bytes, final int offset) {
    return (bytes[offset] & 0xFF) | ((bytes[offset + 1] & 0xFF) << 8);
  }

  private static long readUInt32LE(final byte[] bytes, final int offset) {
    long value = 0;
    for (int i = 0; i < 4; i++) {
      value |= ((long) bytes[offset + i] & 0xFF) << (i * 8);
    }
    return value;
  }
}
