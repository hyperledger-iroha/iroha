// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Immutable canonical 65-byte uncompressed P-256 Kagemusha device public key. */
final class KagemushaDevicePublicKeyV2 {
  private final byte[] value;

  public KagemushaDevicePublicKeyV2(final byte[] sec1Bytes) {
    this.value = KagemushaP256Codec.requireUncompressedPublicKey(sec1Bytes);
  }

  public byte[] sec1Bytes() {
    return value.clone();
  }

  @Override
  public boolean equals(final Object other) {
    return other instanceof KagemushaDevicePublicKeyV2
        && Arrays.equals(value, ((KagemushaDevicePublicKeyV2) other).value);
  }

  @Override
  public int hashCode() {
    return Arrays.hashCode(value);
  }
}
