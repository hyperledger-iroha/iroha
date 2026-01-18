// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

/** CRC64-XZ implementation matching Rust/Python codecs. */
public final class CRC64 {
  private static final long POLY = 0xC96C5795D7870F42L;
  private static final long INIT = 0xFFFFFFFFFFFFFFFFL;
  private static final long XOR_OUT = 0xFFFFFFFFFFFFFFFFL;
  private static final long[] TABLE = buildTable();

  private CRC64() {}

  public static long compute(byte[] data) {
    return compute(data, INIT);
  }

  public static long compute(byte[] data, long initial) {
    long crc = initial;
    for (byte b : data) {
      int index = (int) ((crc ^ (b & 0xFF)) & 0xFF);
      crc = TABLE[index] ^ (crc >>> 8);
    }
    return crc ^ XOR_OUT;
  }

  private static long[] buildTable() {
    long[] table = new long[256];
    for (int i = 0; i < 256; i++) {
      long crc = i;
      for (int bit = 0; bit < 8; bit++) {
        if ((crc & 1L) != 0) {
          crc = (crc >>> 1) ^ POLY;
          continue;
        }
        crc = crc >>> 1;
      }
      table[i] = crc;
    }
    return table;
  }
}
