// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import com.github.luben.zstd.Zstd;

final class NoritoCompression {
  private NoritoCompression() {}

  static byte[] compressZstd(byte[] payload, int level) {
    return Zstd.compress(payload, level);
  }

  static byte[] decompressZstd(byte[] payload, int targetSize) {
    return Zstd.decompress(payload, targetSize);
  }
}
