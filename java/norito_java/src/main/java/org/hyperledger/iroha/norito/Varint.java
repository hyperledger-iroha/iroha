// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.io.ByteArrayOutputStream;
import java.io.IOException;

/** 7-bit little-endian varint helpers. */
public final class Varint {
  private Varint() {}

  public static byte[] encode(long value) {
    if (value < 0) {
      throw new IllegalArgumentException("Varint cannot encode negative values");
    }
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    long remaining = value;
    while (true) {
      int bits = (int) (remaining & 0x7F);
      remaining >>>= 7;
      if (remaining != 0) {
        out.write(bits | 0x80);
      } else {
        out.write(bits);
        break;
      }
    }
    return out.toByteArray();
  }

  public static DecodeResult decode(byte[] data, int offset) {
    long result = 0;
    int shift = 0;
    int index = offset;
    while (true) {
      if (index >= data.length) {
        throw new IllegalArgumentException("Unexpected end of data while decoding varint");
      }
      int b = data[index++] & 0xFF;
      result |= (long) (b & 0x7F) << shift;
      if ((b & 0x80) == 0) {
        break;
      }
      shift += 7;
      if (shift >= 64) {
        throw new IllegalArgumentException("Varint exceeds 64 bits");
      }
    }
    return new DecodeResult(result, index);
  }

  public record DecodeResult(long value, int nextOffset) {}
}
