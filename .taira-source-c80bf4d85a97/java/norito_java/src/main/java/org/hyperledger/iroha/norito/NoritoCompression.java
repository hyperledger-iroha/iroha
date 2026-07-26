// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

final class NoritoCompression {
  private static final boolean HAS_ZSTD;
  private static final Method COMPRESS_METHOD;
  private static final Method DECOMPRESS_METHOD;

  static {
    boolean hasZstd;
    Method compress;
    Method decompress;
    try {
      Class<?> cls = Class.forName("com.github.luben.zstd.Zstd");
      compress = cls.getMethod("compress", byte[].class, int.class);
      decompress = cls.getMethod("decompress", byte[].class, int.class);
      hasZstd = true;
    } catch (ClassNotFoundException | NoSuchMethodException e) {
      hasZstd = false;
      compress = null;
      decompress = null;
    }
    HAS_ZSTD = hasZstd;
    COMPRESS_METHOD = compress;
    DECOMPRESS_METHOD = decompress;
  }

  private NoritoCompression() {}

  static boolean hasZstd() {
    return HAS_ZSTD;
  }

  static byte[] compressZstd(byte[] payload, int level) {
    if (!HAS_ZSTD) {
      throw new UnsupportedOperationException("Zstd compression not available");
    }
    try {
      return (byte[]) COMPRESS_METHOD.invoke(null, payload, level);
    } catch (IllegalAccessException | InvocationTargetException e) {
      throw new RuntimeException("Zstd compression failed", e);
    }
  }

  static byte[] decompressZstd(byte[] payload, int targetSize) {
    if (!HAS_ZSTD) {
      throw new UnsupportedOperationException("Zstd compression not available");
    }
    try {
      return (byte[]) DECOMPRESS_METHOD.invoke(null, payload, targetSize);
    } catch (IllegalAccessException | InvocationTargetException e) {
      throw new RuntimeException("Zstd decompression failed", e);
    }
  }
}
