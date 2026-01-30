// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.util.Objects;

/** Baseline streaming codec helpers aligned with the Rust Norito implementation. */
public final class NoritoStreamingCodec {
  private NoritoStreamingCodec() {}

  public static final int BLOCK_SIZE = 8;
  public static final int BLOCK_PIXELS = BLOCK_SIZE * BLOCK_SIZE;
  public static final int RLE_EOB = 0xFF;
  public static final int MAX_ZERO_RUN = 254;

  private static final int[] ZIG_ZAG = {
      0, 1, 8, 16, 9, 2, 3, 10,
      17, 24, 32, 25, 18, 11, 4, 5,
      12, 19, 26, 33, 40, 48, 41, 34,
      27, 20, 13, 6, 7, 14, 21, 28,
      35, 42, 49, 56, 57, 50, 43, 36,
      29, 22, 15, 23, 30, 37, 44, 51,
      58, 59, 52, 45, 38, 31, 39, 46,
      53, 60, 61, 54, 47, 55, 62, 63
  };

  public sealed interface CodecError
      permits CodecError.RleOverflow, CodecError.TruncatedBlock, CodecError.MissingEndOfBlock {
    int blockIndex();

    record RleOverflow(int blockIndex) implements CodecError {}

    record TruncatedBlock(int blockIndex) implements CodecError {}

    record MissingEndOfBlock(int blockIndex) implements CodecError {}
  }

  public static final class CodecException extends RuntimeException {
    private final CodecError error;

    public CodecException(CodecError error) {
      super(error.toString());
      this.error = Objects.requireNonNull(error, "error");
    }

    public CodecError error() {
      return error;
    }
  }

  public record BlockDecodeResult(short[] coeffs, int offset, short prevDc) {
    public BlockDecodeResult {
      Objects.requireNonNull(coeffs, "coeffs");
      if (coeffs.length != BLOCK_PIXELS) {
        throw new IllegalArgumentException("coeffs must be 64 elements");
      }
    }
  }

  /**
   * Decode a single run-length encoded block.
   *
   * @param bytes source payload
   * @param offset current byte offset
   * @param prevDc previous DC coefficient
   * @param blockIndex block index for error reporting
   * @return decoded coefficients and updated cursor state
   */
  public static BlockDecodeResult decodeBlockRle(
      byte[] bytes, int offset, short prevDc, int blockIndex) {
    Objects.requireNonNull(bytes, "bytes");
    if (offset < 0 || offset > bytes.length) {
      throw new IllegalArgumentException("offset out of range");
    }
    if (offset + 2 > bytes.length) {
      throw new CodecException(new CodecError.TruncatedBlock(blockIndex));
    }
    short[] coeffs = new short[BLOCK_PIXELS];
    short dcDiff = readI16Le(bytes, offset);
    offset += 2;
    short dc = (short) (prevDc + dcDiff);
    coeffs[0] = dc;
    prevDc = dc;

    int pos = 1;
    boolean finished = false;
    while (pos < BLOCK_PIXELS) {
      if (offset + 3 > bytes.length) {
        throw new CodecException(new CodecError.TruncatedBlock(blockIndex));
      }
      int run = bytes[offset] & 0xFF;
      short value = readI16Le(bytes, offset + 1);
      offset += 3;
      if (run == RLE_EOB) {
        finished = true;
        break;
      }
      int advance = run;
      if (pos + advance >= BLOCK_PIXELS) {
        throw new CodecException(new CodecError.RleOverflow(blockIndex));
      }
      pos += advance;
      coeffs[ZIG_ZAG[pos]] = value;
      pos += 1;
    }

    if (!finished) {
      if (pos < BLOCK_PIXELS) {
        throw new CodecException(new CodecError.TruncatedBlock(blockIndex));
      }
      if (offset + 3 <= bytes.length && (bytes[offset] & 0xFF) == RLE_EOB) {
        offset += 3;
        return new BlockDecodeResult(coeffs, offset, prevDc);
      }
      throw new CodecException(new CodecError.MissingEndOfBlock(blockIndex));
    }
    return new BlockDecodeResult(coeffs, offset, prevDc);
  }

  private static short readI16Le(byte[] bytes, int offset) {
    int lo = bytes[offset] & 0xFF;
    int hi = bytes[offset + 1] & 0xFF;
    return (short) (lo | (hi << 8));
  }
}
