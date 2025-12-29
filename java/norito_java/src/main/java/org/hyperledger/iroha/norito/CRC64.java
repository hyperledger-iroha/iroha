// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

/** CRC64-ECMA implementation matching Rust/Python codecs. */
public final class CRC64 {
  private static final long POLY = 0x42F0E1EBA9EA3693L;
  private static final long[] TABLE = buildTable();

  private CRC64() {}

  public static long compute(byte[] data) {
    return compute(data, 0L);
  }

  public static long compute(byte[] data, long initial) {
    long crc = initial;
    for (byte b : data) {
      int index = (int) (((crc >>> 56) ^ (b & 0xFF)) & 0xFF);
      crc = TABLE[index] ^ (crc << 8);
    }
    return crc & 0xFFFFFFFFFFFFFFFFL;
  }

  private static long[] buildTable() {
    long[] table = new long[256];
    for (int i = 0; i < 256; i++) {
      long crc = ((long) i) << 56;
      for (int bit = 0; bit < 8; bit++) {
        boolean carry = (crc & (1L << 63)) != 0;
        crc = (crc << 1) & 0xFFFFFFFFFFFFFFFFL;
        if (carry) {
          crc ^= POLY;
        }
      }
      table[i] = crc;
    }
    return table;
  }
}
